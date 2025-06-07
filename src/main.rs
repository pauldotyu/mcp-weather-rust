use anyhow::Result;
use reqwest;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool,
    transport::stdio,
};
use tracing_subscriber::{self, EnvFilter};

// Set the base URL for the NWS API and user agent
const NWS_API_BASE: &str = "https://api.weather.gov";
const USER_AGENT: &str = "weather-app/1.0";

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

fn format_alerts(alerts: &[Feature]) -> String {
    if alerts.is_empty() {
        return "No active alerts found.".to_string();
    }

    // Pre-allocate capacity for performance
    let mut result = String::with_capacity(alerts.len() * 200);

    // Iterate through each alert and format the output
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

    // Pre-allocate capacity for performance
    let mut result = String::with_capacity(periods.len() * 150);

    // Iterate through each period and format the output
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

#[derive(Debug, Clone)]
pub struct Weather {
    client: reqwest::Client,
}

#[tool(tool_box)]
impl Weather {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");
        Self { client }
    }

    // Helper function to make HTTP requests and handle responses
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

    #[tool(description = "Get weather alerts for a US state")]
    async fn get_alerts(
        &self,
        #[tool(param)]
        #[schemars(description = "the US state to get alerts for")]
        state: String,
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

    #[tool(description = "Get forecast using latitude and longitude coordinates")]
    async fn get_forecast(
        &self,
        #[tool(aggr)] PointsRequest {
            latitude,
            longitude,
        }: PointsRequest,
    ) -> String {
        tracing::info!(
            "Received coordinates: latitude = {}, longitude = {}",
            latitude,
            longitude
        );
        let points_url = format!("{}/points/{},{}", NWS_API_BASE, latitude, longitude);

        // Get the forecast URL
        let points_result = self.make_request::<PointsResponse>(&points_url).await;
        let points = match points_result {
            Ok(points) => points,
            Err(e) => {
                tracing::error!("Failed to fetch points: {}", e);
                return "No forecast found or an error occurred.".to_string();
            }
        };

        // Get the forecast data
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
}

#[tool(tool_box)]
impl ServerHandler for Weather {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A simple weather forecaster".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    tracing::info!("Starting MCP server");

    // Create the Weather service and serve it over stdio
    let service = Weather::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    // Wait for the service to finish
    service.waiting().await?;
    Ok(())
}
