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

/// Example client written in Dart for interacting with KUKSA Databroker,
/// built on the published `kuksa_dart_sdk` — the Dart counterpart to the
/// sibling `kuksa_go_client` / `kuksa-cpp-client`.
///
/// It connects to a running databroker and exercises each core `kuksa.val.v2`
/// API — server info, read, write, metadata listing, and subscribe — then
/// exits cleanly. Start a databroker first (see README.md), then run:
///
///   dart pub get
///   dart run bin/kuksa_dart_client.dart
///
/// Defaults (host `localhost`, port `55555`, signal `Vehicle.Speed`) come from
/// `kuksa-client.json` if present and can be overridden on the command line:
///
///   dart run bin/kuksa_dart_client.dart --host localhost --port 55555 --path Vehicle.Speed
library;

import 'package:kuksa_dart_client/kuksa_dart_client_config.dart';
import 'package:kuksa_dart_sdk/kuksa_dart_sdk.dart';

const String _configFile = 'kuksa-client.json';

Future<void> main(List<String> args) async {
  final config = ClientConfig.fromFile(_configFile).withArgs(args);
  final client = KuksaClient(host: config.host, port: config.port);

  try {
    await client.connect();
    print('Connected to kuksa-databroker at ${config.host}:${config.port}\n');

    await _getServerInfo(client);
    await _getValue(client, config.path);
    await _publishValue(client, config.path);
    await _listMetadata(client, config.path);
    await _subscribe(client, config.path);
  } finally {
    await client.dispose();
  }
}

/// Prints the databroker server name, version, and commit.
Future<void> _getServerInfo(KuksaClient client) async {
  final info = await client.getServerInfo();
  print('[getServerInfo] ${info.name} ${info.version} (${info.commitHash})');
}

/// Reads the current value of a single signal.
Future<void> _getValue(KuksaClient client, String path) async {
  final dp = await client.getValue(path);
  if (dp.hasValue) {
    print('[getValue] $path = ${dp.value}');
  } else {
    print('[getValue] $path has no value yet');
  }
}

/// Publishes (writes) a sample value for the signal.
Future<void> _publishValue(KuksaClient client, String path) async {
  await client.publishValue(path, 100.34);
  print('[publishValue] $path <- 100.34');
}

/// Lists metadata for signals under a path prefix.
Future<void> _listMetadata(KuksaClient client, String path) async {
  final response = await client.listMetadata(filter: path);
  print('[listMetadata] ${response.metadata.length} entry(ies) under $path');
  for (final m in response.metadata) {
    print('  - ${m.path}');
  }
}

/// Subscribes to a signal and prints the first update, then stops.
///
/// The databroker delivers the signal's current value once on subscribe, then
/// only on subsequent change. For a static signal in a short-lived sample we
/// take the first update and stop so the demo terminates cleanly; a
/// long-running app would keep the subscription open and react to each change.
Future<void> _subscribe(KuksaClient client, String path) async {
  print('[subscribe] watching $path (first update)...');
  await for (final update in client.subscribe([path])) {
    final dp = update[path];
    if (dp != null) {
      print('  update: $path = ${dp.value}');
      break;
    }
  }
  print('[subscribe] done');
}
