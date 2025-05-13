/********************************************************************************
 * Copyright (c) 2024 Contributors to the Eclipse Foundation
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

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub zenoh_config: zenoh::config::Config,
    pub kuksa: KuksaConfig,
    pub signals: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct KuksaConfig {
    pub databroker_url: String,
}

#[cfg(test)]
mod tests {
    use super::ProviderConfig;

    #[test]
    fn test_read_config() {
        let conf = r#"
        {
            zenoh_conf: {
                mode: "client",
                connect: {
                    endpoints: [
                        "tcp/zenoh-router:7447",
                    ],
                },
            },
            kuksa: {
                databroker_url: "https://localhost:55555",
            },
            signals: [
                'Vehicle.Body.Horn.IsActive',
                'Vehicle.Body.Horn.IsActive'
            ],
        }
        "#;

        let config: ProviderConfig = json5::from_str(conf).expect("failed to deserialize JSON5");
        assert!(config.signals.len() == 1);
        assert_eq!(config.kuksa.databroker_url, "https://localhost:55555");
        assert_eq!(config.zenoh_config.get_json("mode").unwrap(), "\"client\"");
    }
}
