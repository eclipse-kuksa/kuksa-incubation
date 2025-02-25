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
#include "kuksa/val/v1/types.pb.h"
#include "kuksa/val/v1/val.pb.h"
#include "kuksa/val/v2/types.pb.h"
#include "kuksa/val/v2/val.pb.h"
#include "kuksaclient.h"
#include <grpcpp/client_context.h>
#include <grpcpp/grpcpp.h>
#include <kuksa/val/v2/val.grpc.pb.h>
#include <memory>
#include <spdlog/sinks/stdout_color_sinks.h>
#include <spdlog/spdlog.h>

namespace kuksa {

const std::string loggerName = "kuksaClient";

class KuksaClient::KuksaClientImpl {
public:
  KuksaClientImpl() {
    spdlog::set_level(spdlog::level::debug);
    mLogger = spdlog::stdout_color_mt(loggerName);
    mIsRunning = false;
  }

  ~KuksaClientImpl() {
    if (mIsRunning.load()) {
      mIsRunning.store(false);
      mLogger->info("Stopping active subscription...");

      // Ensure the thread joins if necessary
      if (mSubscribeThread.joinable()) {
        mSubscribeThread.join();
      }
    }
  };

  // --------------------------- V1 APIs -------------------------------

  bool connect_v1(const std::string &server) {
    mLogger->info("Connect V1 called on {}", server);
    mChannel = grpc::CreateChannel(server, grpc::InsecureChannelCredentials());
    mStubV1 = kuksa::val::v1::VAL::NewStub(mChannel);

    auto deadline = std::chrono::system_clock::now() + std::chrono::seconds(2);
    auto connected = mChannel->WaitForConnected(deadline);

    if (!connected) {
      mLogger->debug("Failed to connect to server within deadline");
      return false;
    }

    return true;
  }

  bool get(const std::string &datapoint, kuksa::val::v1::Datapoint &value) {
    mLogger->info("get v1 invoked on {}", datapoint);

    if (!mStubV1) {
      return false;
    }

    grpc::ClientContext context;
    kuksa::val::v1::GetRequest request;
    kuksa::val::v1::GetResponse response;

    grpc::Status status = mStubV1->Get(&context, request, &response);

    if (!status.ok()) {
      mLogger->debug("RPC failed: {}", status.error_message());
      return false;
    }

    return true;
  }

  bool set(const std::string &datapoint,
           const kuksa::val::v1::Datapoint &value) {
    mLogger->info("set v1 invoked on {}", datapoint);

    if (!mStubV1) {
      mLogger->debug("Client not connected");
      return false;
    }

    grpc::ClientContext context;
    kuksa::val::v1::SetRequest request;
    kuksa::val::v1::SetResponse response;

    kuksa::val::v1::EntryUpdate *update = request.add_updates();
    kuksa::val::v1::DataEntry *data_entry = update->mutable_entry();

    // Set the path of the DataEntry (e.g., the datapoint)
    data_entry->set_path(datapoint);

    // Set the value in the DataEntry
    data_entry->mutable_value()->CopyFrom(value);

    grpc::Status status = mStubV1->Set(&context, request, &response);

    if (!status.ok()) {
      mLogger->debug("RPC failed: {}", status.error_message());
      return false;
    }

    return true;
  }

  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV1 callback) {

    std::for_each(datapoints.begin(), datapoints.end(),
                  [this](const std::string &datapoint) {
                    mLogger->info("Starting subscription on {}", datapoint);
                  });

    if (mIsRunning) {
      mLogger->debug("Subscription already active");
    }

    auto ctx = std::make_unique<grpc::ClientContext>();
    kuksa::val::v1::SubscribeRequest request;

    for (const auto &datapoint : datapoints) {
      auto entry = request.add_entries();
      entry->set_path(datapoint);
      entry->set_view(::kuksa::val::v1::VIEW_CURRENT_VALUE);
      entry->add_fields(::kuksa::val::v1::Field::FIELD_VALUE);
      entry->add_fields(::kuksa::val::v1::Field::FIELD_METADATA);
    }

    // Create subscription stream
    auto reader = mStubV1->Subscribe(ctx.get(), request);
    mIsRunning.store(true);

    mSubscribeThread =
        std::thread([this, reader = std::move(reader), ctx = std::move(ctx),
                     callback = std::move(callback)]() {
          kuksa::val::v1::SubscribeResponse response;
          mLogger->debug("in lambda...");

          while (mIsRunning.load()) {
            if (!reader->Read(&response)) {
              mLogger->debug("Stream disconnected");
              break;
            }

            for (const auto &entry : response.updates()) {
              const auto &path = entry.entry().path();
              const auto &datapoint = entry.entry().actuator_target();

              mLogger->debug("Received update for datapoint: {}", path);

              if (callback) {
                callback(path, datapoint);
              }
            }
          }

          // Handle stream completion
          auto status = reader->Finish();
        });

    mSubscribeThread.detach();
  }

  // --------------------------- V2 APIs -------------------------------

  bool connect_v2(const std::string &server) {
    mLogger->info("Connect V2 called on {}", server);
    mChannel = grpc::CreateChannel(server, grpc::InsecureChannelCredentials());
    mStubV2 = kuksa::val::v2::VAL::NewStub(mChannel);

    auto deadline = std::chrono::system_clock::now() + std::chrono::seconds(2);
    auto connected = mChannel->WaitForConnected(deadline);

    if (!connected) {
      mLogger->debug("Failed to connect to server within deadline");
      return false;
    }

    return true;
  }

  bool get(const std::string &datapoint, kuksa::val::v2::Value &value) {
    mLogger->info("get invoked on {}", datapoint);

    if (!mStubV2) {
      return false;
    }

    grpc::ClientContext context;
    kuksa::val::v2::GetValueRequest request;
    kuksa::val::v2::GetValueResponse response;

    request.mutable_signal_id()->set_path(datapoint);

    grpc::Status status = mStubV2->GetValue(&context, request, &response);

    if (!status.ok()) {
      mLogger->debug("RPC failed: {}", status.error_message());
      return false;
    }

    if (!response.has_data_point()) {
      mLogger->debug("Response has no data point");
      return false;
    }

    const auto &data_point = response.data_point();

    if (!data_point.has_value()) {
      mLogger->debug("Data point has no value");
      return false;
    }

    value = data_point.value();

    return true;
  }

  void subscribe(const std::vector<std::string> &datapoints,
                 kuksaCallbackV2 callback) {

    std::for_each(datapoints.begin(), datapoints.end(),
                  [this](const std::string &datapoint) {
                    mLogger->info("Starting subscription on {}", datapoint);
                  });

    if (mIsRunning) {
      mLogger->debug("Subscription already active");
    }

    auto ctx = std::make_unique<grpc::ClientContext>();
    kuksa::val::v2::SubscribeRequest request;

    // Add paths and buffer size to request
    for (const auto &datapoint : datapoints) {
      request.add_signal_paths(datapoint);
    }
    request.set_buffer_size(10);

    // Create subscription stream
    auto reader = mStubV2->Subscribe(ctx.get(), request);
    mIsRunning.store(true);

    mSubscribeThread =
        std::thread([this, reader = std::move(reader), ctx = std::move(ctx),
                     callback = std::move(callback)]() {
          kuksa::val::v2::SubscribeResponse response;

          while (mIsRunning.load()) {
            if (!reader->Read(&response)) {
              mLogger->debug("Stream disconnected");
              break;
            }

            for (const auto &entry : response.entries()) {
              const auto &path = entry.first;
              const auto &datapoint = entry.second;

              mLogger->debug("Received update for datapoint: {}", path);

              if (callback) {
                callback(path, datapoint.value());
              }
            }
          }

          // Handle stream completion
          auto status = reader->Finish();
        });

    mSubscribeThread.detach();
  }

  bool actuate(const std::string &datapoint,
               const kuksa::val::v2::Value &value) {
    mLogger->info("actuate invoked on {}", datapoint);

    if (!mStubV2) {
      mLogger->debug("Client not connected");
      return false;
    }

    grpc::ClientContext context;
    kuksa::val::v2::ActuateRequest request;
    kuksa::val::v2::ActuateResponse response;

    request.mutable_signal_id()->set_path(datapoint);
    *request.mutable_value() = value;

    grpc::Status status = mStubV2->Actuate(&context, request, &response);

    if (!status.ok()) {
      mLogger->debug("RPC failed: {}", status.error_message());
      return false;
    }

    return true;
  }

  bool publishValue(const std::string &datapoint,
                    const kuksa::val::v2::Value &value) {
    mLogger->info("publish invoked on {}", datapoint);

    if (!mStubV2) {
      mLogger->debug("Client not connected");
      return false;
    }

    grpc::ClientContext context;
    kuksa::val::v2::PublishValueRequest request;
    kuksa::val::v2::PublishValueResponse response;

    request.mutable_signal_id()->set_path(datapoint);
    auto *data_point = request.mutable_data_point();
    *data_point->mutable_value() = value;

    // create timestamp
    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(
        now.time_since_epoch());
    auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(
        now.time_since_epoch() - seconds);

    auto *timestamp = data_point->mutable_timestamp();
    timestamp->set_seconds(seconds.count());
    timestamp->set_nanos(nanos.count());

    grpc::Status status = mStubV2->PublishValue(&context, request, &response);

    if (!status.ok()) {
      mLogger->debug("RPC failed: {}", status.error_message());
      return false;
    }

    return true;
  }

private:
  std::shared_ptr<grpc::Channel> mChannel;
  std::unique_ptr<kuksa::val::v1::VAL::Stub> mStubV1;
  std::unique_ptr<kuksa::val::v2::VAL::Stub> mStubV2;
  std::shared_ptr<spdlog::logger> mLogger;
  std::thread mSubscribeThread;
  std::atomic<bool> mIsRunning;
};

// Public interface implementations
KuksaClient::KuksaClient()
    : mKuksaClient(std::make_unique<KuksaClientImpl>()) {}

KuksaClient::~KuksaClient(){};

bool KuksaClient::connect_v1(const std::string &server) {
  return mKuksaClient->connect_v1(server);
}

bool KuksaClient::connect_v2(const std::string &server) {
  return mKuksaClient->connect_v2(server);
}

bool KuksaClient::get(const std::string &datapoint,
                      kuksa::val::v1::Datapoint &value) {
  return mKuksaClient->get(datapoint, value);
}

bool KuksaClient::get(const std::string &datapoint,
                      kuksa::val::v2::Value &value) {
  return mKuksaClient->get(datapoint, value);
}

bool KuksaClient::set(const std::string &datapoint,
                      const kuksa::val::v1::Datapoint &value) {
  return mKuksaClient->set(datapoint, value);
}

void KuksaClient::subscribe(const std::vector<std::string> &datapoints,
                            kuksaCallbackV1 callback) {
  return mKuksaClient->subscribe(datapoints, callback);
}

void KuksaClient::subscribe(const std::vector<std::string> &datapoints,
                            kuksaCallbackV2 callback) {
  return mKuksaClient->subscribe(datapoints, callback);
}

bool KuksaClient::actuate(const std::string &datapoint,
                          const kuksa::val::v2::Value &value) {
  return mKuksaClient->actuate(datapoint, value);
}

bool KuksaClient::publishValue(const std::string &datapoint,
                               const kuksa::val::v2::Value &value) {
  return mKuksaClient->publishValue(datapoint, value);
}
} // namespace kuksa
