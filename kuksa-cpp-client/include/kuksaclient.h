/********************************************************************************
 * Copyright (c) 2022 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License 2.0 which is available at
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/
#ifndef KUKSACLIENT_H
#define KUKSACLIENT_H
#include "kuksa/val/v1/types.pb.h"
#include "kuksa/val/v2/types.pb.h"
#include <kuksa/val/v1/val.grpc.pb.h>
#include <kuksa/val/v2/val.grpc.pb.h>
#include <memory>
#include <string>

namespace kuksa {

using kuksaCallbackV1 = std::function<void(
    const std::string &path, const kuksa::val::v1::Datapoint &value)>;
using kuksaCallbackV2 = std::function<void(const std::string &path,
                                           const kuksa::val::v2::Value &value)>;

class KuksaClient {
public:
  KuksaClient();
  ~KuksaClient();

  bool connect_v1(const std::string &server);
  bool connect_v2(const std::string &server);
  bool get(const std::string &datapoint, kuksa::val::v2::Value &value);
  bool get(const std::string &datapoint, kuksa::val::v1::Datapoint &value);
  bool set(const std::string &datapoint,
           kuksa::val::v1::Datapoint const &value);
  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV1 callback);
  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV2 callback);
  bool actuate(const std::string &datapoint,
               kuksa::val::v2::Value const &value);
  bool publishValue(const std::string &datapoint,
                    kuksa::val::v2::Value const &value);

private:
  class KuksaClientImpl;
  std::unique_ptr<KuksaClientImpl> mKuksaClient;
};
} // namespace kuksa
#endif // KUKSACLIENT_H
