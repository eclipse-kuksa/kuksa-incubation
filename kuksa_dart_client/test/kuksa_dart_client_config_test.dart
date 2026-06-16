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
import 'package:test/test.dart';

void main() {
  group('ClientConfig', () {
    test('uses sensible defaults', () {
      const config = ClientConfig();
      expect(config.host, 'localhost');
      expect(config.port, 55555);
      expect(config.path, 'Vehicle.Speed');
    });

    test('fromJson reads a full config', () {
      final config = ClientConfig.fromJson(<String, dynamic>{
        'host': 'broker.local',
        'port': 12345,
        'path': 'Vehicle.Width',
      });
      expect(config.host, 'broker.local');
      expect(config.port, 12345);
      expect(config.path, 'Vehicle.Width');
    });

    test('fromJson falls back to defaults on missing or malformed fields', () {
      final config = ClientConfig.fromJson(<String, dynamic>{
        'host': '',
        'port': 'not-an-int',
      });
      expect(config.host, 'localhost');
      expect(config.port, 55555);
      expect(config.path, 'Vehicle.Speed');
    });

    test('withArgs overrides host, port, and path', () {
      final config = const ClientConfig().withArgs(
        <String>['--host', 'broker.local', '--port', '9999', '--path', 'Vehicle.Width'],
      );
      expect(config.host, 'broker.local');
      expect(config.port, 9999);
      expect(config.path, 'Vehicle.Width');
    });

    test('withArgs ignores a malformed port and unknown flags', () {
      final config = const ClientConfig().withArgs(
        <String>['--port', 'abc', '--unknown', 'value'],
      );
      expect(config.port, 55555);
      expect(config.host, 'localhost');
    });

    test('withArgs keeps current values when no flags are given', () {
      final config = const ClientConfig(host: 'broker.local').withArgs(<String>[]);
      expect(config.host, 'broker.local');
    });
  });
}
