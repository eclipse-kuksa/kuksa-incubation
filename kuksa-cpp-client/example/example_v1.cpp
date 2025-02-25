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
#include <ostream>
#include <stdio.h>
#include <unistd.h>

#include "kuksa/val/v1/types.pb.h"
#include "kuksaclient.h"

using namespace kuksa;

void handleValue(const kuksa::val::v1::Datapoint &value) {
  switch (value.value_case()) {
  case kuksa::val::v1::Datapoint::ValueCase::kString:
    std::cout << "String value: " << value.string() << std::endl;
    break;
  case kuksa::val::v1::Datapoint::ValueCase::kBool:
    std::cout << "Bool value: " << value.bool_() << std::endl;
    break;
  case kuksa::val::v1::Datapoint::ValueCase::kInt32:
    std::cout << "Int32 value: " << value.int32() << std::endl;
    break;
  case kuksa::val::v1::Datapoint::ValueCase::kFloat:
    std::cout << "Float value: " << value.float_() << std::endl;
    break;
  case kuksa::val::v1::Datapoint::ValueCase::kDouble:
    std::cout << "Double value: " << value.double_() << std::endl;
    break;
  default:
    std::cout << "Unsupported value type" << std::endl;
    break;
  }
}

void on_data_reception_v1(const std::string &path,
                          const kuksa::val::v1::Datapoint &value) {
  std::cout << "Received " << path << std::endl;
  handleValue(value);
}

int main() {
  std::cout << "Starting example for v1 ..." << std::endl;
  KuksaClient instance;
  // connect to databroker
  bool connectionStatus = instance.connect_v1("127.0.0.1:55555");
  printf("Connection is %s \n",
         (connectionStatus == true) ? "Succesfull" : "Failed");

  sleep(2);

  kuksa::val::v1::Datapoint value{};
  // fetch values from databroker
  if (instance.get("Vehicle.Speed", value)) {
    handleValue(value);
  }
  sleep(1);

  // set values into databroker
  value.set_float_(41.4f);
  instance.set("Vehicle.Speed", value);

  // subscribe to data points
  std::vector<std::string> signals = {"Vehicle.Speed", "Vehicle.Width"};
  instance.subscribe(signals, on_data_reception_v1);

  sleep(10);

  return 0;
}
