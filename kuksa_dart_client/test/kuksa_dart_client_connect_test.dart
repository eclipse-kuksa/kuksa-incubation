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

import 'package:kuksa_dart_client/kuksa_dart_client_config.dart';
import 'package:kuksa_dart_sdk/kuksa_dart_sdk.dart';
import 'package:test/test.dart';

/// Requires a running kuksa-databroker on localhost:55555 (the CI workflow in
/// .github/workflows/kuksa_dart_client.yaml starts one before this runs).
/// kuksa_dart_client_config_test.dart covers config parsing without a broker;
/// this file is the one that proves the example itself still talks to one.
void main() {
  test('connects, publishes, and reads back a value from a real databroker',
      () async {
    const config = ClientConfig();
    final client = KuksaClient(host: config.host, port: config.port);

    try {
      await client.connect();

      final info = await client.getServerInfo();
      expect(info.name, isNotEmpty);

      await client.publishValue(config.path, 77.5);
      final dp = await client.getValue(config.path);
      expect(dp.hasValue, isTrue);
      expect(dp.value, 77.5);
    } finally {
      await client.dispose();
    }
  }, timeout: const Timeout(Duration(seconds: 15)));
}
