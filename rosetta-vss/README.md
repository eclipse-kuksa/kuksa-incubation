# 🗿 Rosetta VSS

**AI-Powered Automotive Signal to VSS Mapper** using **hybrid retrieval (Dense + BM25)**, confidence-based **routing**, and an optional **LLM-as-Judge** stage (bounded to retrieved candidates).

This project maps signals from **DBC**, **C/C++ code**, and **YAML** into standard **Vehicle Signal Specification (VSS)** paths.

## Repository structure

```
ui-related
  app.py                    --- main UI
  app_modules/              --- UI components
    config.py               --- UI Config
    logic.py                --- Core logic flow for UI
    state.py                --- Session State maintenance
    ui.py                   --- Streamlit UI material
  vss_mapper                --- core concept
    core.py                 --- Hybrid Retrieval logic
    domain_rules.py         --- Lexical handling as per Automotive naming
    exporters.py            --- Export file generators
    extractor.py            --- Extract signal names
    llm.py                  --- All LLM related code
    parsers.py              --- Input file parsing
  data                      --- input sample data files (dummy data, openDBC)
  pyproject.toml            --- requirements file for the project (use uv)

```

> Both UI and evaluation depend on `vss_mapper` (the core library that provides `VSSKnowledgeBase`, `auto_parse`, hybrid retrieval and the LLM wrappers).

---

## Core concept

### 1) Parse → Query signals
Input artifacts are parsed into a uniform query representation:
- `name`
- `description`
- `unit`
- `context`

### 2) Hybrid retrieval (Dense + BM25)
For each query signal, the system retrieves **Top‑K** candidate VSS paths using:
- **Dense embeddings** (semantic similarity)
- **BM25** (lexical matching)
- **Fusion** (RRF / weighted)

### 3) Confidence routing
Let:
- `T` = user/eval threshold
- `HighConf` = `kb.config.high_confidence_threshold`

Routing policy:
- `top1_score < T` → **Proposal** (open‑set / manual review)
- `top1_score >= HighConf` → **Auto‑accept top‑1**
- `T <= top1_score < HighConf` → **LLM-as-Judge**

### 4) LLM-as-Judge (bounded)
When invoked, the judge receives the signal + Top‑K candidates and returns:
- **confirm** (choose top‑1)
- **override** (choose another candidate)
- **veto** (no candidate fits → proposal)

**Important:** the judge is constrained to select only from the provided candidate set; any out‑of‑set path is treated as a veto.

---

## Setup

### Python environment
Create an environment and install dependencies.

Using `uv`:
```bash
uv venv
uv sync
```
Make a requirements file for
Using `pip`:
```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```
## Run the Streamlit UI

From the base directory:

```bash
uv run streamlit run app.py
```

In the sidebar, load a VSS standard (the UI provides pre-configured download links). Then:
1. Upload an artifact (`.dbc`, `.c/.cpp/.h`, `.yaml`, etc.)
2. Adjust the **Confidence Threshold** slider (`T`)
3. Click **Run Mapping**

Key behaviors (implemented in `app_modules/logic.py`):
- Top‑K retrieval is performed with `kb.search_batch(..., top_k=5)`
- Mid-band items will be sent to the LLM judge (when `HF_TOKEN` is set)
- Judge may **override** the retrieval match, or **veto** and route to proposal
- Proposal generation is used for open‑set signals

Logs are written to:
- `/logs/app.log`

## KUKSA integration

  The `vss_mapper.exporters.generate_kuksa_config` exporter produces a JSON config in the format expected by the KUKSA
  CAN Provider — i.e. a `general.mapping` list of `{vss_path, can_signal, transform?, interval_ms?}` entries. Drop the
  output into your CAN Provider configuration to ingest the mapped signals into a running Databroker.

---

## Troubleshooting

### “Proposal even with high retrieval confidence”
This is expected when:
- the signal falls into the **judge band** (`T <= score < HighConf`)
- and the LLM judge **vetoes** (returns null/none or low confidence)

The UI should label these as **LLM Veto** (not “low confidence”).

### LLM judge not running
- Ensure `HF_TOKEN` is set
- Ensure the selected config enables `llm_judge`

---

## License
Apache License 2.0. See the LICENSE file at the root of the kuksa-incubation repository.

## Developer and Maintainer
Akshay Narla, IAS, Uni Stuttgart (akshay.narla@ias.uni-stuttgart.de)
