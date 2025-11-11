# Example provider
#
# SPDX-License-Identifier: Apache-2.0
#

import random

from kuksa_client.grpc import Datapoint
from kuksa_client.grpc import VSSClient
import argparse


def updateValue(client, path, value):
    print(f"Updating {path} to: {value}\n")
    client.set_current_values(
        {path: Datapoint(value)})


def choosePad():
    user_input = input("Choose a brake pad (FL, FR, RL, RR): ").upper()
    if user_input == "FL":
        return "Vehicle.Chassis.Axle.Row1.Wheel.Left.Brake.PadWear"
    elif user_input == "FR":
        return "Vehicle.Chassis.Axle.Row1.Wheel.Right.Brake.PadWear"
    elif user_input == "RL":
        return "Vehicle.Chassis.Axle.Row2.Wheel.Left.Brake.PadWear"
    elif user_input == "RR":
        return "Vehicle.Chassis.Axle.Row2.Wheel.Right.Brake.PadWear"
    else:
        print("Invalid input. Defaulting to FL.")
        return "Vehicle.Chassis.Axle.Row1.Wheel.Left.Brake.PadWear"


def chooseValue():
    try:
        user_input = int(input("Choose a value (0-100): "))
        if 0 <= user_input <= 100:
            return user_input
        else:
            print("Value out of range. Defaulting to 0")
            return 0
    except ValueError:
        print("Invalid input. Using a random value instead.")
        return random.randint(0, 100)


def main():
    # Create a VSSClient instance, use 55555 when not on MAC
    parser = argparse.ArgumentParser(description="Break Pad Wear Provider")
    parser.add_argument('-p', '--port',
                        type=int,
                        default=55555,
                        help='Port at the Databroker to connect to (default: 55555)')
    args = parser.parse_args()
    client = VSSClient('127.0.0.1', args.port)
    client.connect()
    while True:
        # Use user input. In real use cases my process CAN messages or similar
        path = choosePad()
        value = chooseValue()
        updateValue(client, path, value)


if __name__ == '__main__':
    main()
