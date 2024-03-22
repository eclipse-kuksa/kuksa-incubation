/********************************************************************************
* Copyright (c) 2023 Contributors to the Eclipse Foundation
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

package kuksa_client

import (
	"log"
	"strconv"

	"github.com/spf13/viper"
)

type KuksaClientConfig struct {
	ServerAddress     string `mapstructure:"serverAddress"`
	ServerPort        string `mapstructure:"serverPort"`
	Insecure          bool   `mapstructure:"insecure"`
	CertsDir          string `mapstructure:"certsDir"`
	TokenOrTokenfile         string `mapstructure:"TokenOrTokenfile"`
	TransportProtocol string `mapstructure:"protocol"`
}

func ReadConfig(config *KuksaClientConfig) {

	// Read in the configuration of the switcher
	log.Println("Reading Config ...")

	viper.SetConfigName("kuksa-client") // name of config file (without extension)
	viper.AddConfigPath("./")           // path to look for the config file in

	viper.SetEnvPrefix("kuksa_client")
	viper.AutomaticEnv()
	err := viper.ReadInConfig() // Find and read the config file
	if err != nil {             // Handle errors reading the config file
		log.Panicf("Fatal error config file: %s \n", err)
	}

	err = viper.Unmarshal(&config)
	if err != nil {
		log.Panicf("Unable to decode config into struct, %v", err)
	}
}

func (config KuksaClientConfig) String() string {

	var retString string

	retString += "\nKuksa VISS Client Config\n"
	retString += "\tServer Address: " + config.ServerAddress + "\n"
	retString += "\tServer Port: " + config.ServerPort + "\n"
	retString += "\tInsecure: " + strconv.FormatBool(config.Insecure) + "\n"
	retString += "\tCertsDir: " + config.CertsDir + "\n"
	retString += "\tTokenOrTokenfile: " + config.TokenOrTokenfile + "\n"
	retString += "\tTransport Protocol: " + string(config.TransportProtocol) + "\n"

	return retString
}
