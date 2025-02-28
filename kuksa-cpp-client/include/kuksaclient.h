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
#include "kuksa/val/v2/val.pb.h"
#include <kuksa/val/v1/val.grpc.pb.h>
#include <kuksa/val/v2/val.grpc.pb.h>
#include <memory>
#include <string>

namespace kuksa {

using kuksaCallbackV1 = std::function<void(
    const std::string &path, const kuksa::val::v1::Datapoint &value)>;

/**
 * @brief Callback function type for v2 Kuksa data subscriptions.
 *
 * This callback is invoked when a subscribed data point's value changes.
 *
 * @param path The VSS name of the data point.
 * @param value The new value of the data point (v2 format).
 */
using kuksaCallbackV2 = std::function<void(const std::string &path,
                                           const kuksa::val::v2::Value &value)>;

/**
 * @class KuksaClient
 * @brief Client class for interacting with the Kuksa data broker.
 *
 * This class provides methods to connect and interact with a Kuksa databroker
 * server.
 */
class KuksaClient {
public:
  KuksaClient();
  ~KuksaClient();

  bool connect_v1(const std::string &server);
  bool get(const std::string &datapoint, kuksa::val::v1::Datapoint &value);
  bool set(const std::string &datapoint,
           kuksa::val::v1::Datapoint const &value);
  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV1 callback);

  /**
   * @brief Connects to a Kuksa server using the v2 API.
   *
   * @param server The server address (ex: "localhost:55555").
   * @return True if the connection was successful, false otherwise.
   */
  bool connect_v2(const std::string &server);

  /**
   * @brief Retrieves the value of a data point using the v2 API.
   *
   * @param datapoint VSS name of the datapoints.
   * @param value The retrieved data point value (v2 format).
   * @return True if the retrieval was successful, false otherwise.
   */
  bool getValue(const std::string &datapoint, kuksa::val::v2::Value &value);

  /**
   * @brief Retrieves the values of a set of data points using the v2 API.
   *
   * @param datapoints A vector of VSS datapoint names.
   * @return A vector of retrieved data point values (v2 format).
   */
  std::vector<kuksa::val::v2::Datapoint>
  getValues(const std::vector<std::string> &datapoints);

  /**
   * @brief Subscribes to updates of a set of data points using the v2 API.
   *
   * @param datapoints A vector of VSS data point names.
   * @param callback The callback function to be invoked when a subscribed data
   * point changes.
   */
  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV2 callback);

  /**
   * @brief Actuates a data point using the v2 API.
   * i.e set the target value of a VSS point.
   * The actuation can fail if there is no provider registered in the broker
   *
   * @param datapoint The VSS name of the data point.
   * @param value The value to set (v2 format).
   * @return True if the actuation was successful, false otherwise.
   */
  bool actuate(const std::string &datapoint,
               kuksa::val::v2::Value const &value);

  /**
   * @brief Publishes a value using the v2 API.
   * i.e. set the current value of a VSS point
   *
   * @param datapoint The VSS name of the data point.
   * @param value The value to publish (v2 format).
   * @return True if the publish operation was successful, false otherwise.
   */
  bool publishValue(const std::string &datapoint,
                    kuksa::val::v2::Value const &value);

  /**
   * @brief Retrieves server information using the v2 API.
   *
   * @param response The server information response.
   * @return True if the operation was successful, false otherwise.
   */
  bool getServerInfo(kuksa::val::v2::GetServerInfoResponse &response);

private:
  class KuksaClientImpl;
  std::unique_ptr<KuksaClientImpl> mKuksaClient;
};
} // namespace kuksa
#endif // KUKSACLIENT_H
