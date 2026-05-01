# ********************************************************************************
# Copyright (c) 2026 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
#
# This program and the accompanying materials are made available under the
# terms of the Apache License 2.0 which is available at
# http://www.apache.org/licenses/LICENSE-2.0
#
# SPDX-License-Identifier: Apache-2.0
# *******************************************************************************/

"""Rosetta VSS: Hybrid retrieval + reranking knowledge base for mapping signals to VSS.+

- Handles text handling from the VSS to form a corpus for the retrieval system.
- Core flow of the retreival+reranking mechanism
- Score calculation:
    Dense: cosine similarity → scores (-1 to 1) → rank top-k: catches semantic similarity
    Sparse: BM25 → scores → rank top-k: catches exact terms embeddings miss
    RRF: combines by rank position → selects candidates (scores discarded) : combines both without
    tuning weights on scores
    Reranker: cross-encoder scores candidate pairs → raw logits: accurate pairwise judgment on small
    candidate set (expensive but precise)
    Prior: ±small adjustment for unit/position match → clipped 0-1:  domain rules embeddings can't learn
    Final: sigmoid(reranker logit) → 0-1 confidence
"""

from __future__ import annotations

import logging
import re
import time
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
import torch
from sentence_transformers import CrossEncoder, SentenceTransformer, util

# Import domain-specific abbreviations for automotive specific embeddings
from .domain_rules import ABBREVIATIONS as DOMAIN_ABBREVIATIONS

try:
    from rank_bm25 import BM25Okapi
    HAS_BM25 = True
except ImportError:
    BM25Okapi = None
    HAS_BM25 = False


class _BM25OkapiLite:
    """Lightweight BM25 fallback when rank_bm25 is not installed."""

    def __init__(self, corpus_tokens, k1: float = 1.5, b: float = 0.75):
        import math
        self.k1 = float(k1)
        self.b = float(b)
        self.corpus_tokens = corpus_tokens
        self.N = len(corpus_tokens)
        self.doc_len = [len(toks) for toks in corpus_tokens]
        self.avgdl = (sum(self.doc_len) / self.N) if self.N else 0.0

        self.tf = []
        df = {}
        for toks in corpus_tokens:
            d = {}
            for t in toks:
                d[t] = d.get(t, 0) + 1
            self.tf.append(d)
            for t in d.keys():
                df[t] = df.get(t, 0) + 1
        self.idf = {}
        for t, n in df.items():
            self.idf[t] = math.log(1 + (self.N - n + 0.5) / (n + 0.5))

    def get_scores(self, query_tokens):
        scores = np.zeros(self.N, dtype=np.float32)
        if self.N == 0:
            return scores
        for i in range(self.N):
            dl = self.doc_len[i]
            denom_const = self.k1 * (1 - self.b + self.b * (dl / (self.avgdl + 1e-9)))
            doc_tf = self.tf[i]
            s = 0.0
            for t in query_tokens:
                if t not in doc_tf:
                    continue
                tf = doc_tf[t]
                idf = self.idf.get(t, 0.0)
                s += idf * (tf * (self.k1 + 1.0)) / (tf + denom_const)
            scores[i] = s
        return scores


logger = logging.getLogger(__name__)


@dataclass
class VSSSignal:
    """Flattened VSS signal representation."""
    path: str
    description: str
    unit: str
    type: str
    datatype: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {
            "path": self.path,
            "description": self.description,
            "unit": self.unit,
            "type": self.type,
            "datatype": self.datatype,
        }


@dataclass
class SearchResult:
    """Result from VSS search."""
    score: float
    vss_signal: VSSSignal
    rank: int = 0
    debug: Optional[Dict[str, Any]] = None

    def to_dict(self) -> Dict[str, Any]:
        d = {"score": self.score, "vss_signal": self.vss_signal.to_dict(), "rank": self.rank}
        if self.debug is not None:
            d["debug"] = self.debug
        return d


@dataclass
class QuerySignal:
    """Input signal to be mapped."""
    name: str
    description: str = ""
    unit: str = ""
    context: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {"name": self.name, "description": self.description,
                "unit": self.unit, "context": self.context}


@dataclass
class HybridConfig:
    """Configuration for hybrid retrieval.

    Simplified confidence model:
      - Score is normalized to [0, 1] based on the model output
      - For cosine similarity: already in [-1, 1], shift to [0, 1]
      - For cross-encoder logits: apply sigmoid
      - Priors (unit/position) add small bonuses to ranking score
    """
    # Candidate selection
    semantic_weight: float = 0.7
    lexical_weight: float = 0.3
    initial_candidates: int = 50
    rrf_k: int = 60

    # Models
    retriever_model: str = "sentence-transformers/all-MiniLM-L6-v2"
    reranker_model: Optional[str] = "cross-encoder/ms-marco-MiniLM-L-6-v2"
    batch_size: int = 64

    # Confidence thresholds (for UI routing)
    high_confidence_threshold: float = 0.7
    medium_confidence_threshold: float = 0.69
    low_confidence_threshold: float = 0.20          # Default for UI slider; can be overridden

    # Prior bonuses (additive, scale 0-1)
    unit_match_bonus: float = 0.05
    unit_mismatch_penalty: float = 0.10
    position_mismatch_penalty: float = 0.08


class VSSKnowledgeBase:
    """Hybrid retrieval + reranking system for VSS signal mapping."""

    def __init__(
        self,
        vss_data: Dict[str, Any],
        config: Optional[HybridConfig] = None,
        device: Optional[str] = None,
    ):
        self.config = config or HybridConfig()
        self.device = device or ("cuda" if torch.cuda.is_available() else "cpu")

        logger.info("Initializing VSSKnowledgeBase on %s", self.device)
        logger.info("Config: semantic=%.2f, lexical=%.2f, candidates=%d",
                    self.config.semantic_weight, self.config.lexical_weight,
                    self.config.initial_candidates)

        # Load retriever
        logger.info("Loading retriever: %s", self.config.retriever_model)
        t0 = time.time()
        self.retriever = SentenceTransformer(self.config.retriever_model, device=self.device)
        logger.info("Retriever loaded in %.2fs", time.time() - t0)

        # Load reranker (optional)
        self.reranker: Optional[CrossEncoder] = None
        if self.config.reranker_model:
            logger.info("Loading reranker: %s", self.config.reranker_model)
            t0 = time.time()
            self.reranker = CrossEncoder(
                self.config.reranker_model,
                device=self.device,
                default_activation_function=torch.nn.Identity(),
            )
            logger.info("Reranker loaded in %.2fs", time.time() - t0)

        # Flatten VSS tree
        logger.info("Flattening VSS specification...")
        t0 = time.time()
        self.signals: List[VSSSignal] = self._flatten_spec(vss_data)
        if not self.signals:
            raise ValueError("No valid signals in VSS specification")
        logger.info("Flattened %d signals in %.2fs", len(self.signals), time.time() - t0)

        # Build corpus texts
        self.corpus_text = [self._signal_to_text(s) for s in self.signals]
        self.corpus_rerank_text = [self._signal_to_rerank_text(s) for s in self.signals]
        self._path_tokens = [self._expand_abbreviations(self._tokenize_text(s.path)) for s in self.signals]

        # Format for specific retrievers
        self.corpus_embed_text = [self._format_for_retriever(t, is_query=False) for t in self.corpus_text]

        # Compute embeddings
        logger.info("Computing corpus embeddings...")
        t0 = time.time()
        self.corpus_embeddings = self.retriever.encode(
            self.corpus_embed_text,
            convert_to_tensor=True,
            show_progress_bar=True,
            batch_size=self.config.batch_size,
            device=self.device,
        )
        logger.info("Embeddings computed in %.2fs, shape=%s", time.time() - t0, self.corpus_embeddings.shape)

        # BM25 index
        self.bm25 = None
        if self.config.lexical_weight > 0:
            logger.info("Building BM25 index...")
            t0 = time.time()
            tokenized = [self._tokenize_for_bm25(doc) for doc in self.corpus_text]
            if HAS_BM25 and BM25Okapi is not None:
                self.bm25 = BM25Okapi(tokenized)
                logger.info("BM25Okapi ready in %.2fs", time.time() - t0)
            else:
                self.bm25 = _BM25OkapiLite(tokenized)
                logger.info("BM25OkapiLite (fallback) ready in %.2fs", time.time() - t0)

        logger.info("Knowledge base ready: %d signals indexed", len(self.signals))

    # ------------ Text formatting ---------------

    def _format_for_retriever(self, text: str, is_query: bool) -> str:
        """Model-specific formatting for dense retrievers."""
        model = (self.config.retriever_model or "").lower()
        if "e5" in model:
            return ("query: " if is_query else "passage: ") + text
        if "bge" in model and is_query:
            return "Represent this sentence for searching relevant passages: " + text
        return text

    _TOKEN_RE = re.compile(r"[A-Za-z0-9]+")
    _CAMEL_BOUNDARY_RE = re.compile(r"(?<=[a-z])(?=[A-Z0-9])|(?<=[A-Z])(?=[A-Z][a-z])")

    # Use comprehensive domain-specific abbreviations
    # to-do: currently, some specific manually added abbreviations
    _ABBR_EXPANSIONS: Dict[str, List[str]] = DOMAIN_ABBREVIATIONS

    _UNIT_ALIASES: Dict[str, str] = {
        "kmh": "km/h", "kph": "km/h", "km/hr": "km/h", "mps": "m/s",
        "c": "degc", "°c": "degc", "celsius": "degc", "kelvin": "k",
        "f": "degf", "°f": "degf", "degree": "deg", "degrees": "deg",
        "percent": "%", "percentage": "%", "volt": "v", "volts": "v",
        "amp": "a", "amps": "a"
    }

    _UNIT_BUCKETS: Dict[str, set] = {
        "speed": {"km/h", "mph", "m/s"},
        "temperature": {"degc", "degf", "k"},
        "angle": {"deg", "rad"},
        "percent": {"%"},
        "pressure": {"pa", "kpa", "bar", "psi"},
        "voltage": {"v"},
        "current": {"a"},
        "torque": {"nm"},
        "rotational": {"rpm"},
    }

    @staticmethod
    def _split_camel(text: str) -> str:
        return VSSKnowledgeBase._CAMEL_BOUNDARY_RE.sub(" ", text)

    @staticmethod
    def _tokenize_text(text: str) -> List[str]:
        if not text:
            return []
        t = text.replace(".", " ").replace("_", " ").replace("/", " ").replace("-", " ").replace(":", " ")
        t = VSSKnowledgeBase._split_camel(t)
        raw = VSSKnowledgeBase._TOKEN_RE.findall(t)
        tokens: List[str] = []
        for w in raw:
            parts = re.findall(r"[A-Za-z]+|[0-9]+", w)
            for p in parts:
                lp = p.lower()
                if lp:
                    tokens.append(lp)
        return tokens

    @staticmethod
    def _expand_abbreviations(tokens: List[str]) -> List[str]:
        expanded: List[str] = []
        for t in tokens:
            expanded.append(t)
            ext = VSSKnowledgeBase._ABBR_EXPANSIONS.get(t)
            if ext:
                expanded.extend(ext)
        return expanded

    @staticmethod
    def _tokenize_for_bm25(text: str) -> List[str]:
        return VSSKnowledgeBase._expand_abbreviations(VSSKnowledgeBase._tokenize_text(text))

    @classmethod
    def _normalize_unit(cls, unit: str) -> str:
        if not unit:
            return ""
        u = unit.strip().lower().replace(" ", "")
        return cls._UNIT_ALIASES.get(u, u)

    @classmethod
    def _unit_bucket(cls, unit: str) -> Optional[str]:
        u = cls._normalize_unit(unit)
        if not u:
            return None
        for bucket, units in cls._UNIT_BUCKETS.items():
            if u in units:
                return bucket
        return None

    @classmethod
    def _unit_compatibility(cls, query_unit: str, candidate_unit: str) -> int:
        """Return 1 if compatible, -1 if incompatible, 0 if unknown."""
        qb = cls._unit_bucket(query_unit)
        cb = cls._unit_bucket(candidate_unit)
        if qb is None or cb is None:
            return 0
        return 1 if qb == cb else -1

    @staticmethod
    def _position_tokens(tokens: List[str]) -> set:
        return {t for t in tokens if t in {"front", "rear", "left", "right", "row1", "row2", "1", "2"}}

    @staticmethod
    def _signal_to_text(signal: VSSSignal) -> str:
        path_tokens = VSSKnowledgeBase._expand_abbreviations(VSSKnowledgeBase._tokenize_text(signal.path))
        parts = [f"Path: {signal.path}", f"Tokens: {' '.join(path_tokens)}"]
        if signal.description:
            parts.append(f"Description: {signal.description}")
        if signal.unit:
            parts.append(f"Unit: {signal.unit}")
        if signal.type:
            parts.append(f"Type: {signal.type}")
        return " ".join(parts)

    @staticmethod
    def _signal_to_rerank_text(signal: VSSSignal) -> str:
        parts = [f"VSS: {signal.path}"]
        if signal.description:
            parts.append(signal.description)
        if signal.unit:
            parts.append(f"[{signal.unit}]")
        return " | ".join(parts)

    def _flatten_spec(self, data: Dict[str, Any]) -> List[VSSSignal]:
        signals: List[VSSSignal] = []

        def recurse(node: Dict[str, Any], path: str) -> None:
            if not isinstance(node, dict):
                return
            node_type = node.get("type", "").lower()
            if node_type in ("sensor", "actuator", "attribute"):
                unit = node.get("unit", "")
                datatype = node.get("datatype", "")
                if isinstance(datatype, dict):
                    unit = unit or datatype.get("unit", "")
                    datatype = datatype.get("type", "")
                signals.append(VSSSignal(
                    path=path, description=node.get("description", ""),
                    unit=unit, type=node_type, datatype=str(datatype)
                ))
            for key, child in node.get("children", {}).items():
                recurse(child, f"{path}.{key}" if path else key)

        for key, value in data.items():
            recurse(value, str(key))
        return signals

    def _build_query(self, signal: QuerySignal) -> str:
        name_tokens = self._expand_abbreviations(self._tokenize_text(signal.name))
        parts = [f"Signal: {signal.name}", f"Tokens: {' '.join(name_tokens)}"]
        if signal.description:
            parts.append(f"Description: {signal.description}")
        if signal.unit:
            parts.append(f"Unit: {signal.unit}")
        if signal.context:
            parts.append(f"Context: {signal.context}")
        return " ".join(parts)

    def _build_reranker_query(self, signal: QuerySignal) -> str:
        name_tokens = self._expand_abbreviations(self._tokenize_text(signal.name))
        parts = [f"{signal.name} ({' '.join(name_tokens)})" if name_tokens else signal.name]
        if signal.description:
            parts.append(signal.description)
        if signal.unit:
            parts.append(f"[{signal.unit}]")
        return " | ".join(parts)

    # Penalize/Bonus the confidence scores if there are mismatches in the units/positions
    # in the extracted signals
    # to-do: generalize for other signals than VSS
    def _compute_prior(self, query: QuerySignal, query_tokens: List[str], doc_idx: int) -> float:
        """Compute prior adjustment based on unit and position compatibility."""
        prior = 0.0

        # Unit compatibility
        comp = self._unit_compatibility(query.unit or "", self.signals[doc_idx].unit or "")
        if comp == 1:
            prior += self.config.unit_match_bonus
        elif comp == -1:
            prior -= self.config.unit_mismatch_penalty

        # Position compatibility
        q_pos = self._position_tokens(query_tokens)
        c_pos = self._position_tokens(self._path_tokens[doc_idx])

        # Check for position conflicts (mainly for differentiating positions)
        if ("left" in q_pos and "right" in c_pos) or ("right" in q_pos and "left" in c_pos):
            prior -= self.config.position_mismatch_penalty
        if ("front" in q_pos and "rear" in c_pos) or ("rear" in q_pos and "front" in c_pos):
            prior -= self.config.position_mismatch_penalty
        # Row number check (also very specific to VSS now)
        if ("1" in q_pos or "row1" in q_pos) and ("2" in c_pos or "row2" in c_pos):
            prior -= self.config.position_mismatch_penalty
        if ("2" in q_pos or "row2" in q_pos) and ("1" in c_pos or "row1" in c_pos):
            prior -= self.config.position_mismatch_penalty

        # Position match bonus (if query has position and candidate matches)
        if q_pos and c_pos:
            if q_pos & c_pos:  # intersection
                prior += 0.03  # small bonus for position match

        return prior

    def search_batch(self, signals: List[QuerySignal], top_k: int = 5) -> List[List[SearchResult]]:
        """Batch search with simplified confidence scoring."""
        if not signals:
            return []

        n_queries = len(signals)
        logger.info("BATCH SEARCH: %d queries, top_k=%d", n_queries, top_k)

        # Build queries in a format suitable for the models
        query_base = [self._build_query(s) for s in signals]
        query_embed = [self._format_for_retriever(q, is_query=True) for q in query_base]

        # Encode queries
        t0 = time.time()
        query_embeddings = self.retriever.encode(
            query_embed, convert_to_tensor=True, show_progress_bar=True,
            batch_size=self.config.batch_size, device=self.device,
        )
        encode_time = time.time() - t0

        # Dense similarity
        t0 = time.time()
        dense_scores = util.cos_sim(query_embeddings, self.corpus_embeddings).cpu().numpy()
        sim_time = time.time() - t0

        k0 = min(self.config.initial_candidates, len(self.signals))
        rrf_k = max(1, int(self.config.rrf_k))

        candidate_docs: List[List[int]] = []
        candidate_dense: List[Dict[int, float]] = []
        candidate_sparse: List[Dict[int, float]] = []  # BM25 scores per candidate

        # Candidate selection via Reciprocal Rank Fusion (RRF): Combine dense and sparse and then get rankings
        # https://learn.microsoft.com/en-us/azure/search/hybrid-search-ranking
        t0 = time.time()
        for q_idx in range(n_queries):
            q_dense = dense_scores[q_idx]

            # Top-k dense
            if k0 < len(q_dense):
                dense_idx = np.argpartition(-q_dense, k0 - 1)[:k0]
                dense_idx = dense_idx[np.argsort(q_dense[dense_idx])[::-1]]
            else:
                dense_idx = np.argsort(q_dense)[::-1]

            # get lexical score for the signals based on text. lower weighted.
            q_sparse_scores = None
            sparse_idx = np.array([], dtype=int)
            if self.bm25 is not None and self.config.lexical_weight > 0:
                q_tokens = self._tokenize_for_bm25(query_base[q_idx])
                q_sparse_scores = self.bm25.get_scores(q_tokens)
                if k0 < len(q_sparse_scores):
                    sparse_idx = np.argpartition(-q_sparse_scores, k0 - 1)[:k0]
                    sparse_idx = sparse_idx[np.argsort(q_sparse_scores[sparse_idx])[::-1]]
                else:
                    sparse_idx = np.argsort(q_sparse_scores)[::-1]

            # RRF fusion
            fused: Dict[int, float] = {}
            if self.config.semantic_weight > 0:
                for rank, doc_idx in enumerate(dense_idx, start=1):
                    di = int(doc_idx)
                    fused[di] = fused.get(di, 0.0) + (self.config.semantic_weight / (rrf_k + rank))

            if q_sparse_scores is not None and self.config.lexical_weight > 0:
                for rank, doc_idx in enumerate(sparse_idx, start=1):
                    di = int(doc_idx)
                    fused[di] = fused.get(di, 0.0) + (self.config.lexical_weight / (rrf_k + rank))

            if fused:
                chosen = [doc for doc, _ in sorted(fused.items(), key=lambda x: x[1], reverse=True)[:k0]]
            else:
                chosen = [int(i) for i in dense_idx[:k0]]

            candidate_docs.append(chosen)
            candidate_dense.append({doc: float(q_dense[doc]) for doc in chosen})
            # Store BM25 scores for chosen candidates (for hybrid scoring without reranker)
            if q_sparse_scores is not None:
                max_sp = float(q_sparse_scores.max()) if q_sparse_scores.max() > 0 else 1.0
                candidate_sparse.append({doc: float(q_sparse_scores[doc]) / max_sp for doc in chosen})
            else:
                candidate_sparse.append({})

        fusion_time = time.time() - t0

        # Cross-encoder reranking
        rerank_time = 0.0
        rerank_scores_by_query: List[Dict[int, float]] = [dict() for _ in range(n_queries)]

        if self.reranker is not None:
            query_rerank = [self._build_reranker_query(s) for s in signals]
            rerank_pairs: List[List[str]] = []
            pair_map: List[Tuple[int, int]] = []
            for q_idx, docs in enumerate(candidate_docs):
                qtxt = query_rerank[q_idx]
                for doc_idx in docs:
                    rerank_pairs.append([qtxt, self.corpus_rerank_text[doc_idx]])
                    pair_map.append((q_idx, doc_idx))

            # pair the extracted signals and rerank them based on the score
            t0 = time.time()
            if rerank_pairs:
                rerank_raw = self.reranker.predict(rerank_pairs, show_progress_bar=True)
                for i, (q_idx, doc_idx) in enumerate(pair_map):
                    rerank_scores_by_query[q_idx][doc_idx] = float(rerank_raw[i])
            rerank_time = time.time() - t0

        # Final ranking and scoring
        final_results: List[List[SearchResult]] = []

        for q_idx, q in enumerate(signals):
            docs = candidate_docs[q_idx]
            if not docs:
                final_results.append([])
                continue

            q_tokens = self._expand_abbreviations(
                self._tokenize_text((q.name or "") + " " + (q.description or "") + " " + (q.context or ""))
            )

            # Compute scores
            scores = []
            for doc_idx in docs:
                if self.reranker is not None:
                    # Cross-encoder: sigmoid to normalize and get scores between 0 and 1.
                    # rerank score already brings it to a simple calculation
                    raw = rerank_scores_by_query[q_idx].get(doc_idx, -10.0)
                    base_score = 1.0 / (1.0 + np.exp(-raw))
                else:
                    # Cosine similarity: shift from [-1,1] to [0,1]
                    raw = candidate_dense[q_idx].get(doc_idx, 0.0)
                    dense_score = (raw + 1.0) / 2.0

                    # Blend with BM25 if available (hybrid mode)
                    sparse_score = candidate_sparse[q_idx].get(doc_idx, 0.0)
                    if sparse_score > 0 and self.config.lexical_weight > 0:
                        base_score = (self.config.semantic_weight * dense_score
                                      + self.config.lexical_weight * sparse_score)
                    else:
                        base_score = dense_score

                # Add prior adjustments: bonus or penalty to the score based on the units.
                # this is just an extra step. Can be removed also.
                prior = self._compute_prior(q, q_tokens, doc_idx)
                final_score = np.clip(base_score + prior, 0.0, 1.0)

                scores.append((doc_idx, final_score, base_score, prior))

            # Sort by final score
            scores.sort(key=lambda x: x[1], reverse=True)
            ranked = scores[:top_k]

            results: List[SearchResult] = []
            for r, (doc_idx, final_score, base_score, prior) in enumerate(ranked, start=1):
                results.append(SearchResult(
                    score=final_score,
                    vss_signal=self.signals[doc_idx],
                    rank=r,
                    debug={
                        "base_score": base_score,
                        "prior_adjustment": prior,
                        "used_reranker": self.reranker is not None,
                        "sparse_score": candidate_sparse[q_idx].get(doc_idx, 0.0) if candidate_sparse else 0.0,
                    },
                ))

            final_results.append(results)

        total_time = encode_time + sim_time + fusion_time + rerank_time
        logger.info("search_batch: %d queries in %.2fs (%.1f/sec)",
                    n_queries, total_time, n_queries / max(1e-9, total_time))
        return final_results

    def get_confidence_level(self, score: float) -> str:
        """Interpret confidence score."""
        if score >= self.config.high_confidence_threshold:
            return "high"
        if score >= self.config.medium_confidence_threshold:
            return "medium"
        if score >= self.config.low_confidence_threshold:
            return "low"
        return "very_low"

    def select_llm_indices(
        self, batch_results, proposal_threshold
    ) -> List[int]:
        """Return indices of signals in mid-confidence band (for LLM judge)."""
        return [
            i for i, res in enumerate(batch_results)
            if res and proposal_threshold <= res[0].score < self.config.high_confidence_threshold
        ]

    def dump_tokenization_diagnostic(
        self,
        signals: List[QuerySignal],
        output_path: str = "tokenization_diagnostic",
    ) -> None:
        """
        For every signal being mapped, show three tokenization views side-by-side:
          1. BGE-M3 subword tokens  — what the dense encoder actually sees
          2. Custom split           — _tokenize_text() before abbreviation expansion
          3. BM25 expanded          — after _expand_abbreviations(), what BM25 indexes

        Writes both .csv and .md files to output_path (no extension needed).
        Called from logic.py right after the QuerySignal list is built.
        """
        import csv
        from pathlib import Path

        # ── Get the tokenizer from the already-loaded retriever ──────────
        # SentenceTransformer exposes the underlying HF tokenizer at
        # self.retriever.tokenizer (all ST versions) or via the first module.
        tokenizer = None
        try:
            tokenizer = self.retriever.tokenizer
        except AttributeError:
            try:
                tokenizer = self.retriever[0].auto_model.config  # fallback
            except Exception:
                pass

        # Determine the model-specific query prefix so we strip it correctly
        model = (self.config.retriever_model or "").lower()
        if "bge" in model:
            prefix = "Represent this sentence for searching relevant passages: "
        elif "e5" in model:
            prefix = "query: "
        else:
            prefix = ""

        prefix_len = 0
        if tokenizer and prefix:
            try:
                prefix_len = len(tokenizer.encode(prefix, add_special_tokens=False))
            except Exception:
                prefix_len = 0

        rows = []
        for q in signals:
            # ── Path 1: BGE-M3 (or configured retriever) subword tokens ──
            bge_tokens: List[str] = []
            if tokenizer:
                try:
                    full_ids = tokenizer.encode(
                        prefix + q.name, add_special_tokens=False
                    )
                    all_tokens = tokenizer.convert_ids_to_tokens(full_ids)
                    bge_tokens = all_tokens[prefix_len:]
                except Exception as e:
                    bge_tokens = [f"<tokenizer error: {e}>"]

            # ── Path 2: Custom rule-based split (pre-expansion) ───────────
            raw_tokens = self._tokenize_text(q.name)

            # ── Path 3: After abbreviation expansion (what BM25 indexes) ──
            expanded_tokens = self._expand_abbreviations(raw_tokens)

            rows.append({
                "signal":          q.name,
                "unit":            q.unit or "",
                "context":         q.context or "",
                "bge_tokens":      bge_tokens,
                "bge_n":           len(bge_tokens),
                "raw_tokens":      raw_tokens,
                "raw_n":           len(raw_tokens),
                "expanded_tokens": expanded_tokens,
                "expanded_n":      len(expanded_tokens),
            })

        stem = Path(output_path)

        # ── CSV ───────────────────────────────────────────────────────────
        csv_path = stem.with_suffix(".csv")
        with open(csv_path, "w", newline="", encoding="utf-8") as f:
            writer = csv.writer(f)
            writer.writerow([
                "signal", "unit", "context",
                "bge_subword_tokens", "bge_n",
                "custom_split", "custom_n",
                "bm25_expanded", "bm25_n",
            ])
            for r in rows:
                writer.writerow([
                    r["signal"], r["unit"], r["context"],
                    " | ".join(r["bge_tokens"]),  r["bge_n"],
                    " | ".join(r["raw_tokens"]),  r["raw_n"],
                    " | ".join(r["expanded_tokens"]), r["expanded_n"],
                ])
        logger.info("Tokenization diagnostic saved: %s", csv_path)

        # ── Markdown ──────────────────────────────────────────────────────
        md_path = stem.with_suffix(".md")
        with open(md_path, "w", encoding="utf-8") as f:
            f.write("# Tokenization Diagnostic\n\n")
            f.write(f"Model: `{self.config.retriever_model}`  \n")
            f.write(f"Signals processed: {len(rows)}\n\n")
            f.write(
                "| Signal | Unit | Context | "
                "BGE-M3 subword (`n`) | "
                "Custom split (`n`) | "
                "BM25 expanded (`n`) |\n"
            )
            f.write("|--------|------|---------|---------------------|"
                    "-------------------|--------------------|\n")
            for r in rows:
                bge_fmt = " · ".join(r["bge_tokens"]) if r["bge_tokens"] else "*(no tokenizer)*"
                raw_fmt = " · ".join(r["raw_tokens"])
                exp_fmt = " · ".join(r["expanded_tokens"])
                f.write(
                    f"| `{r['signal']}` | {r['unit']} | {r['context']} | "
                    f"{bge_fmt} (`{r['bge_n']}`) | "
                    f"{raw_fmt} (`{r['raw_n']}`) | "
                    f"{exp_fmt} (`{r['expanded_n']}`) |\n"
                )
        logger.info("Tokenization diagnostic saved: %s", md_path)
