# MCP Server for Weather

This is a simple Model Context Protocol (MCP) server that provides weather data built using the _soon-to-be-released_ [rust-sdk](https://github.com/modelcontextprotocol/rust-sdk).

This walkthrough takes inspiration from the [quickstart guides for server developers](https://modelcontextprotocol.io/quickstart/server), which can be found on the [Model Context Protocol website](https://modelcontextprotocol.io/), and builds on the examples provided in the [rust-sdk MCP server examples](https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples/servers).

## TL;DR

Want to skip the walkthrough and just run the weather server? No problem!

Clone this repository, and run the following commands to build and run the weather server:

```sh
cargo run
```

Then, in a separate terminal, run the MCP Inspector to test the server:

```sh
npx @modelcontextprotocol/inspector
```

This will start the MCP server and the MCP Inspector, allowing you to interact with the server and test its capabilities.

## Prerequisites

Before you begin, ensure you have the following installed:

- [Rust and Cargo](https://www.rust-lang.org/tools/install) installed on your machine.
- [Node.js 22+ and npm](https://nodejs.org/en/download) installed for running the [MCP inspector](https://github.com/modelcontextprotocol/inspector).

## Project setup

Create a new Rust project:

```sh
cargo new weather
cd weather
```

The following dependencies will need to be added to your `Cargo.toml` file.

- **[rmcp](https://crates.io/crates/rmcp)**: The MCP SDK for Rust.
- **[tokio](https://crates.io/crates/tokio)**: For asynchronous runtime and I/O operations.
- **[serde](https://crates.io/crates/serde)**: For serializing and deserializing data structures.
- **[serde_json](https://crates.io/crates/serde_json)**: For JSON serialization and deserialization.
- **[anyhow](https://crates.io/crates/anyhow)**: For error handling.
- **[tracing](https://crates.io/crates/tracing)**: For logging and diagnostics.
- **[tracing-subscriber](https://crates.io/crates/tracing-subscriber)**: For subscribing to tracing events and filtering logs.
- **[reqwest](https://crates.io/crates/reqwest)**: For making HTTP requests to the National Weather Service API.
- **[axum](https://crates.io/crates/axum)**: For building the HTTP server and routing requests.

You can add these dependencies manually to your `Cargo.toml` file, or you can use the `cargo add` command to add them easily.

```sh
cargo add rmcp@0.2.0 --features server,transport-sse-server,transport-io,transport-streamable-http-server,auth
cargo add tokio --features macros,rt-multi-thread,signal
cargo add serde --features derive
cargo add serde_json
cargo add anyhow
cargo add tracing
cargo add tracing-subscriber --features env-filter
cargo add reqwest --features json
cargo add axum --features macros
```

Here is how your `Cargo.toml` should look:

```toml
[package]
name = "weather"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1.0.98"
axum = { version = "0.8.4", features = ["macros"] }
reqwest = { version = "0.12.22", features = ["json"] }
rmcp = { version = "0.2.0", features = ["auth", "server", "transport-io", "transport-sse-server", "transport-streamable-http-server"] }
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.140"
tokio = { version = "1.46.0", features = ["macros", "rt-multi-thread", "signal"] }
tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["env-filter"] }
```

Open the project in your favorite code editor.

## Building the weather server

The Weather server will provide weather data for a given location. The server will provide two tools:

1. `get_alerts`: Returns weather alerts for a given state.
2. `get_forecast`: Returns the weather forecast for a given location (location is provided as latitude and longitude coordinates).

Open the `src/main.rs` file and add following code to the top of the file:

```rust
use anyhow::Result;
use reqwest;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpService, session::local::LocalSessionManager,
    },
};

use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    {self},
};

const NWS_API_BASE: &str = "https://api.weather.gov";
const USER_AGENT: &str = "weather-app/2.0";
const BIND_ADDRESS: &str = "127.0.0.1:8000";
```

This code imports the necessary crates and modules for building the server.

> [!NOTE]
> The `src/main.rs` has a main function that you will implement later to run the server. Leave it as is for now and add code above the main function.

## Testing NWS API endpoints

To retrieve weather data, the server will make HTTP requests to the [National Weather Service (NWS) API](https://www.weather.gov/documentation/services-web-api).

This RESTful API has several endpoints that allow you to access weather data without requiring an API key. All that is required is to set a user agent in the HTTP request headers.

The following endpoints will be used in this project:

- **Alerts Endpoint**: `https://api.weather.gov/alerts/active?area={state}` - This endpoint returns active weather alerts for a given state.
- **Points Endpoint**: `https://api.weather.gov/points/{latitude},{longitude}` - This endpoint returns the forecast URL for a specific latitude and longitude.
- **Forecast Endpoint**: `https://api.weather.gov/gridpoints/{office}/{gridX},{gridY}/forecast` - This endpoint returns the weather forecast for a specific grid point.

To test these endpoints manually, you can use the `curl` command or any HTTP client to make requests to the NWS API.

For example, to get weather alerts for a state, you can use the following command:

```sh
curl "https://api.weather.gov/alerts/active?area=CA"
```

To get the weather forecast, you would need to make
a request to the points endpoint for a specific location. In the response, you will receive a forecast URL which you can use to get the forecast data.

For example, to get the forecast for Los Angeles, you can use the following command:

```sh
curl -L "https://api.weather.gov/points/34.0499998,-118.249999"
```

> [!NOTE]
> The `-L` flag in the `curl` command is used to follow redirects, as the NWS API normalizes precise location points to a more general location, for internal purposes.

Within the points response, you will find a forecast field that contains the URL for the forecast data.

Use the forecast URL to get the forecast data:

```sh
curl "https://api.weather.gov/gridpoints/LOX/155,45/forecast"
```

Within the forecast response, you will find a properties field that contains the forecast data, which includes an array of periods with details about the weather forecast for different times of the day.

## Modeling the weather data

To work with the weather data returned by the NWS API, define Rust structs that match the structure of the JSON data returned by the API. This will allow the server to deserialize the JSON data into Rust types using the `serde` crate.

Add the following code to the `src/main.rs` file to define the structs for the weather data:

```rust
#[derive(Debug, serde::Deserialize)]
pub struct AlertResponse {
    pub features: Vec<Feature>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Feature {
    pub properties: FeatureProps,
}

#[derive(Debug, serde::Deserialize)]
pub struct FeatureProps {
    pub event: String,
    #[serde(rename = "areaDesc")]
    pub area_desc: String,
    pub severity: String,
    pub status: String,
    pub headline: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PointsRequest {
    #[schemars(description = "latitude of the location in decimal format")]
    pub latitude: String,
    #[schemars(description = "longitude of the location in decimal format")]
    pub longitude: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PointsResponse {
    pub properties: PointsProps,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PointsProps {
    pub forecast: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GridPointsResponse {
    pub properties: GridPointsProps,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GridPointsProps {
    pub periods: Vec<Period>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct Period {
    pub name: String,
    pub temperature: i32,
    #[serde(rename = "temperatureUnit")]
    pub temperature_unit: String,
    #[serde(rename = "windSpeed")]
    pub wind_speed: String,
    #[serde(rename = "windDirection")]
    pub wind_direction: String,
    #[serde(rename = "shortForecast")]
    pub short_forecast: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetAlertsRequest {
    #[schemars(description = "the US state to get alerts for")]
    pub state: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetForecastRequest {
    #[schemars(description = "latitude of the location in decimal format")]
    pub latitude: String,
    #[schemars(description = "longitude of the location in decimal format")]
    pub longitude: String,
}
```

These structs represent the data returned by the NWS API for weather alerts and forecasts and how parameters for the tool's function calls will be defined.

## Adding helper functions

The server will return weather alerts and forecasts in a human-readable format. To achieve this, implement helper functions to format the data.

Add the following helper functions to the `src/main.rs` file:

```rust
fn format_alerts(alerts: &[Feature]) -> String {
    if alerts.is_empty() {
        return "No active alerts found.".to_string();
    }

    let mut result = String::with_capacity(alerts.len() * 200);

    for alert in alerts {
        result.push_str(&format!(
            "Event: {}\nArea: {}\nSeverity: {}\nStatus: {}\nHeadline: {}\n---\n",
            alert.properties.event,
            alert.properties.area_desc,
            alert.properties.severity,
            alert.properties.status,
            alert.properties.headline
        ));
    }
    result
}

fn format_forecast(periods: &[Period]) -> String {
    if periods.is_empty() {
        return "No forecast data available.".to_string();
    }

    let mut result = String::with_capacity(periods.len() * 150);

    for period in periods {
        result.push_str(&format!(
            "Name: {}\nTemperature: {}°{}\nWind: {} {}\nForecast: {}\n---\n",
            period.name,
            period.temperature,
            period.temperature_unit,
            period.wind_speed,
            period.wind_direction,
            period.short_forecast
        ));
    }
    result
}
```

## Implementing the weather tools

Add the following code to define a `Weather` struct that will hold the HTTP client used to make requests to the NWS API, and implement the tools for getting alerts and forecasts.

```rust
#[derive(Debug, Clone)]
pub struct Weather {
    tool_router: ToolRouter<Self>,
    client: reqwest::Client,
}
```

Next, create the `Weather` struct which is where the tools for getting weather alerts and forecasts will be implemented.

```rust
#[tool_router]
impl Weather {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            client: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}
```

This code creates a new instance of the `Weather` struct with an HTTP client that has the user agent set to `weather-app/1.0`. The client is a reusable instance that will be used to make requests to the NWS API.

As demonstrated above, there will be a few HTTP requests made to the NWS API. To make the code cleaner and more reusable, create a `make_request` function to the `Weather` struct implementation:

```rust
async fn make_request<T>(&self, url: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
{
    tracing::info!("Making request to: {}", url);

    let response = self
        .client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    tracing::info!("Received response: {:?}", response);

    match response.status() {
        reqwest::StatusCode::OK => response
            .json::<T>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e)),
        status => Err(format!("Request failed with status: {}", status)),
    }
}
```

This function takes a URL as input, makes an HTTP GET request to that URL, and returns the deserialized response as the specified type `T`. If the request fails or the response cannot be parsed, it returns an error message.

Add the following functions to the `Weather` struct implementation to implement the `get_alerts` tool.

```rust
#[tool(description = "Get weather alerts for a US state")]
async fn get_alerts(
    &self,
    Parameters(GetAlertsRequest { state }): Parameters<GetAlertsRequest>,
) -> String {
    tracing::info!("Received request for weather alerts in state: {}", state);

    let url = format!("{}/alerts/active?area={}", NWS_API_BASE, state);

    match self.make_request::<AlertResponse>(&url).await {
        Ok(alerts) => format_alerts(&alerts.features),
        Err(e) => {
            tracing::error!("Failed to fetch alerts: {}", e);
            "No alerts found or an error occurred.".to_string()
        }
    }
}
```

This function retrieves weather alerts for a specified US state. It constructs the URL for the NWS API, makes the request, and formats the alerts into a human-readable string.

Next, implement the `get_forecast` tool to retrieve the weather forecast using latitude and longitude coordinates. Add the following function to the `Weather` struct implementation:

```rust
#[tool(description = "Get forecast using latitude and longitude coordinates")]
async fn get_forecast(
    &self,
    Parameters(GetForecastRequest {
        latitude,
        longitude,
    }): Parameters<GetForecastRequest>,
) -> String {
    tracing::info!(
        "Received coordinates: latitude = {}, longitude = {}",
        latitude,
        longitude
    );

    let points_url = format!("{}/points/{},{}", NWS_API_BASE, latitude, longitude);

    let points_result = self.make_request::<PointsResponse>(&points_url).await;

    let points = match points_result {
        Ok(points) => points,
        Err(e) => {
            tracing::error!("Failed to fetch points: {}", e);
            return "No forecast found or an error occurred.".to_string();
        }
    };

    match self
        .make_request::<GridPointsResponse>(&points.properties.forecast)
        .await
    {
        Ok(forecast) => format_forecast(&forecast.properties.periods),
        Err(e) => {
            tracing::error!("Failed to fetch forecast: {}", e);
            "No forecast found or an error occurred.".to_string()
        }
    }
}
```

This function retrieves the weather forecast for a specific location using latitude and longitude coordinates. It first makes a request to the points endpoint to get the forecast URL, then uses that URL to fetch the actual forecast data.

Next, implement the `ServerHandler` trait for the `Weather` struct as well to provide server information and capabilities. Add the following code to the `src/main.rs` file:

```rust
#[tool_handler]
impl ServerHandler for Weather {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A simple weather forecaster".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
```

> [!NOTE]
> This is another implementation on the `Weather` struct.

Finally, implement the `main` function to run the server. Replace the existing `main` function in `src/main.rs` with the following code:

```rust
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let service = StreamableHttpService::new(
        || Ok(Weather::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
        .await;
    Ok(())
}
```

This code initializes the tracing subscriber for logging, creates an instance of the `Weather` service, and starts the MCP server using standard input/output (stdio) transport. The server will listen for requests and respond with weather data.

Run the following commands to format the code and build the project:

```sh
cargo fmt
cargo run
```

If all goes well, you should see no errors, and the server will be build successfully and running.

## Testing with MCP Inspector

Test the MCP server with the [Model Context Protocol Inspector](https://github.com/modelcontextprotocol/inspector). The inspector is a web-based tool that allows you to interact with MCP servers and test their capabilities.

In a separate terminal, run the MCP Inspector using the following command:

```sh
npx @modelcontextprotocol/inspector
```

Once the MCP Inspector is started, navigate to URL listed in the terminal output and you should see the MCP Inspector interface.

## Summary

In this walkthrough, you built a simple MCP server that provides weather data using the Model Context Protocol. This is a great starting point for building more complex MCP servers that can provide various types of data and services. Be sure to check out the [Rust SDK examples](https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples/servers) for more advanced use cases and features.

## Learn more

- [Model Context Protocol (MCP) official docs](https://modelcontextprotocol.io/)
- [RMCP](https://github.com/modelcontextprotocol/rust-sdk)
- [MCP Inspector](https://github.com/modelcontextprotocol/inspector)
