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

type KuksaBackend interface {
	ConnectToKuksaVal() error
	Close()
	AuthorizeKuksaValConn(TokenOrTokenfile string) error
	GetValueFromKuksaVal(path string, attr string) ([]interface{}, error)
	SetValueFromKuksaVal(path string, value string, attr string) error
	SubscribeFromKuksaVal(path string, attr string) (string, error)
	UnsubscribeFromKuksaVal(id string) error
	PrintSubscriptionMessages(id string) error
	GetMetadataFromKuksaVal(path string) ([]interface{}, error)
}
