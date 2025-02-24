#################################################################################
# Copyright (c) 2023 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
#
# This program and the accompanying materials are made available under the
# terms of the Apache License 2.0 which is available at
# http://www.apache.org/licenses/LICENSE-2.0
#
# SPDX-License-Identifier: Apache-2.0
#################################################################################

import os
import sys
import signal
import threading
import configparser
from kuksa_client.grpc import VSSClient
from kuksa_client.grpc import Datapoint
from telemetry_f1_2021.listener import TelemetryListener
from threading import Lock

scriptDir = os.path.dirname(os.path.realpath(__file__))
sys.path.append(os.path.join(scriptDir, "../../"))

# telemetry packet ids
TelemetryPacketID_Engine = 6
TelemetryPacketID_CarStatus = 7
TelemetryPacketID_CarDamage = 10
TelemetryPacketID_LapTime = 2

class Kuksa_Client():
    # Constructor
    def __init__(self, config):
        print("Init kuksa client...")
        self.config = config
        if "kuksa_val" not in config:
            print("kuksa_val section missing from configuration, exiting")
            sys.exit(-1)
        kuksaConfig = config['kuksa_val']
        self.host = kuksaConfig.get('host')
        self.port = kuksaConfig.getint('port')

    def shutdown(self):
        self.client.stop()

    # Christophers approach on sending Data to Kuksa Server
    def setTelemetryData(self, telemetryData):
        with VSSClient(self.host, self.port) as client:
            client.set_current_values(telemetryData)


class carTelemetry_Client():

    def __init__(self, config, consumer):
        print("Init carTelemetry client...")
        self.consumer = consumer
        if "listenerIPAddr" not in config:
            print("listenerIPAddr section missing from configuration, exiting")
            sys.exit(-1)
        if "PS5_UDPPort" not in config:
            print("PS5_UDPPort section missing from configuration, exiting")
            sys.exit(-1)
        # init thread variables
        self.datastructure_lock = Lock()
        self.list_for_Ids = []
        self.id_To_LastPacket = {}
        self.id_To_ProcessingFunction = {}
        self.id_To_ProcessingFunction[TelemetryPacketID_Engine] = (
            self.processTelemetryPacket_Engine
        )
        self.id_To_ProcessingFunction[TelemetryPacketID_CarDamage] = (
            self.processTelemetryPacket_CarDamage
        )
        self.id_To_ProcessingFunction[TelemetryPacketID_CarStatus] = (
            self.processTelemetryPacket_CarStatus
        )
        self.id_To_ProcessingFunction[TelemetryPacketID_LapTime] = (
            self.processTelemetryPacket_LapTime
        )
        # extract carTelemetry Data
        print("Connecting to extract CarTelemetry Data")

        self.carTelemetry = {}
        self.running = True

        self.thread = threading.Thread(target=self.loop, args=())
        self.thread.start()

    def get_next_packet(self):
        print("start next packet thread")
        while self.running:
            try:
                # listen to the data via UDP channel
                packet = self.listener.get()
                packetID = packet.m_header.m_packet_id
                if packetID in [
                    TelemetryPacketID_Engine,
                    TelemetryPacketID_CarDamage,
                    TelemetryPacketID_CarStatus,
                    TelemetryPacketID_LapTime,
                ]:
                    with self.datastructure_lock:
                        if not packetID in self.list_for_Ids:
                            self.list_for_Ids.append(packetID)
                        self.id_To_LastPacket[packetID] = packet
            except Exception:
                continue

    def loop(self):
        print("Car Telemetry data loop started")

        # extract config
        config_ipAddr = config['listenerIPAddr']
        config_UDPport = config['PS5_UDPPort']
        listener_ip = config_ipAddr['host']
        udp_port = config_UDPport['port']

        print(f"listener_ip:{listener_ip}")
        print(f"udp_port:{udp_port}")

        # init threads to process telemetry data
        self.listener = TelemetryListener(port=int(udp_port), host=listener_ip)
        thread_get_next_packet = threading.Thread(target=self.get_next_packet)
        thread_initPacketProcessing = threading.Thread(target=self.initPacketProcessing)
        thread_get_next_packet.start()
        thread_initPacketProcessing.start()
        thread_get_next_packet.join()
        thread_initPacketProcessing.join()

    def processTelemetryPacket_Engine(self, telemetryPacket):
        # Get data
        carIndex = telemetryPacket.m_header.m_player_car_index
        Speed = telemetryPacket.m_car_telemetry_data[carIndex].m_speed
        EngineRPM = telemetryPacket.m_car_telemetry_data[carIndex].m_engine_rpm
        # Store data
        carTelemetry = {}
        carTelemetry['Vehicle.Speed'] = Datapoint(Speed)
        carTelemetry['Vehicle.RPM'] = Datapoint(EngineRPM)
        return carTelemetry

    def processTelemetryPacket_CarDamage(self, telemetryPacket):
        # Get data
        carIndex = telemetryPacket.m_header.m_player_car_index
        leftWingDamage = telemetryPacket.m_car_damage_data[carIndex].m_front_left_wing_damage
        rightWingDamage = telemetryPacket.m_car_damage_data[carIndex].m_front_right_wing_damage
        # Extract nested data
        tyreWear_1 = telemetryPacket.m_car_damage_data[carIndex].m_tyres_wear[0]
        tyreWear_2 = telemetryPacket.m_car_damage_data[carIndex].m_tyres_wear[1]
        tyreWear_3 = telemetryPacket.m_car_damage_data[carIndex].m_tyres_wear[2]
        tyreWear_4 = telemetryPacket.m_car_damage_data[carIndex].m_tyres_wear[3]
        # Store data
        carTelemetry = {}
        carTelemetry['Vehicle.FrontLeftWingDamage'] = Datapoint(leftWingDamage)
        carTelemetry['Vehicle.FrontRightWingDamage'] = Datapoint(rightWingDamage)
        carTelemetry['Vehicle.Tire.RearLeftWear'] = Datapoint(tyreWear_1)
        carTelemetry['Vehicle.Tire.RearRightWear'] = Datapoint(tyreWear_2)
        carTelemetry['Vehicle.Tire.FrontLeftWear'] = Datapoint(tyreWear_3)
        carTelemetry['Vehicle.Tire.FrontRightWear'] = Datapoint(tyreWear_4)
        return carTelemetry

    def processTelemetryPacket_LapTime(self, telemetryPacket):
        # Get data
        carIndex = telemetryPacket.m_header.m_player_car_index
        lastLapTime_in_ms = telemetryPacket.m_lap_data[carIndex].m_last_lap_time_in_ms
        # Preprocessing
        lastLapTime_in_s = lastLapTime_in_ms / 1000
        # Store data
        carTelemetry = {}
        carTelemetry['Vehicle.LastLapTime'] = Datapoint(lastLapTime_in_s)
        return carTelemetry

    def processTelemetryPacket_CarStatus(self, telemetryPacket):
        # Get data
        carIndex = telemetryPacket.m_header.m_player_car_index
        fuelInTank = telemetryPacket.m_car_status_data[carIndex].m_fuel_in_tank
        fuelCapacity = telemetryPacket.m_car_status_data[carIndex].m_fuel_capacity
        # Preprocessing
        fuelInPercent = int((fuelInTank / fuelCapacity) * 100)
        # Store data
        carTelemetry = {}
        carTelemetry['Vehicle.FuelLevel'] = Datapoint(fuelInPercent)
        return carTelemetry

    def initPacketProcessing(self):
        while self.running:
            try:
                # Initializing variables
                packetID = None
                packet = None
                # Lock datastructures
                with self.datastructure_lock:
                    # Update packet ID and get dependend packet
                    if len(self.list_for_Ids) > 0:
                        packetID = self.list_for_Ids.pop()
                        packet = self.id_To_LastPacket[packetID]
                    else:
                        packetID = None
                        packet = None
                if packet is not None:
                    # Process telemetry packet
                    carTelemetry = self.id_To_ProcessingFunction[packetID](packet)
                    # Forward data to KUKSA_VAL
                    self.consumer.setTelemetryData(carTelemetry)
            except Exception:
                continue

    def shutdown(self):
        self.running = False
        self.consumer.shutdown()
        self.carTelemetry.close()
        self.thread.join()


if __name__ == "__main__":
    print("<kuksa.val> Car Telemetry example feeder")
    config_candidates = ['/config/carTelemetry_feeder.ini',
                         '/etc/carTelemetry_feeder.ini',
                         os.path.join(scriptDir, 'config/carTelemetry_feeder.ini')]
    for candidate in config_candidates:
        if os.path.isfile(candidate):
            configfile = candidate
            break
    if configfile is None:
        print("No configuration file found. Exiting")
        sys.exit(-1)
    config = configparser.ConfigParser()
    config.read(configfile)

    client = carTelemetry_Client(config, Kuksa_Client(config))

    def terminationSignalreceived(signalNumber, frame):
        print("Received termination signal. Shutting down")
        client.shutdown()
    signal.signal(signal.SIGINT, terminationSignalreceived)
    signal.signal(signal.SIGQUIT, terminationSignalreceived)
    signal.signal(signal.SIGTERM, terminationSignalreceived)

# end of file #
