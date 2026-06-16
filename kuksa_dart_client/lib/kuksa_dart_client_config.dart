// Copyright (c) 2026 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache License 2.0 which is available at
// http://www.apache.org/licenses/LICENSE-2.0
//
// SPDX-License-Identifier: Apache-2.0

import 'dart:convert';
import 'dart:io';

/// Connection + signal configuration for the KUKSA Dart example client.
///
/// Mirrors the configuration model of the sibling `kuksa_go_client` (which
/// reads `kuksa-client.json`): a databroker [host] and [port], plus the signal
/// [path] the sample exercises. Values may come from a JSON config file and/or
/// command-line flags, with command-line flags taking precedence. Missing or
/// malformed values fall back to defaults so the sample is runnable out of the
/// box without any configuration.
class ClientConfig {
  const ClientConfig({
    this.host = 'localhost',
    this.port = 55555,
    this.path = 'Vehicle.Speed',
  });

  /// Databroker host. Default `localhost`.
  final String host;

  /// Databroker gRPC port. Default `55555`.
  final int port;

  /// Signal path the sample reads/writes/subscribes. Default `Vehicle.Speed`.
  final String path;

  /// Builds a config from a decoded JSON map, falling back to the default for
  /// any missing or wrongly-typed field (the sample must run on a partial or
  /// empty config rather than crash).
  factory ClientConfig.fromJson(Map<String, dynamic> json) {
    const defaults = ClientConfig();
    final host = json['host'];
    final port = json['port'];
    final path = json['path'];
    return ClientConfig(
      host: host is String && host.isNotEmpty ? host : defaults.host,
      port: port is int && port > 0 ? port : defaults.port,
      path: path is String && path.isNotEmpty ? path : defaults.path,
    );
  }

  /// Reads [filePath] if it exists, otherwise returns defaults. Never throws on
  /// a missing file so the sample runs out of the box.
  static ClientConfig fromFile(String filePath) {
    final file = File(filePath);
    if (!file.existsSync()) {
      return const ClientConfig();
    }
    final decoded = jsonDecode(file.readAsStringSync());
    if (decoded is! Map<String, dynamic>) {
      return const ClientConfig();
    }
    return ClientConfig.fromJson(decoded);
  }

  /// Applies `--host`, `--port`, `--path` overrides on top of this config.
  /// Unknown flags are ignored; a malformed `--port` keeps the current port.
  ClientConfig withArgs(List<String> args) {
    var result = this;
    for (var i = 0; i < args.length - 1; i++) {
      final flag = args[i];
      final value = args[i + 1];
      if (flag == '--host') {
        result = result.copyWith(host: value);
      } else if (flag == '--port') {
        final parsed = int.tryParse(value);
        if (parsed != null && parsed > 0) {
          result = result.copyWith(port: parsed);
        }
      } else if (flag == '--path') {
        result = result.copyWith(path: value);
      }
    }
    return result;
  }

  ClientConfig copyWith({String? host, int? port, String? path}) => ClientConfig(
        host: host ?? this.host,
        port: port ?? this.port,
        path: path ?? this.path,
      );

  @override
  String toString() => 'ClientConfig(host: $host, port: $port, path: $path)';
}
