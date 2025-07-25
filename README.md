# PDU Exporter

The **PDU Exporter** is a lightweight custom Prometheus exporter designed to collect metrics from PDU (Power Distribution Unit) devices that expose raw status data via the `/status.cgi` endpoint over HTTP. It transforms this data into Prometheus-compatible metrics.

## Features

* Connects directly to the PDU via raw TCP (port 80)
* Sends a manual HTTP GET request with basic authentication
* Exposes metrics such as current, voltage, power, energy, temperature, and humidity
* Dockerized for easy deployment

## Exported Metrics

| Metric Name    | Description               | Labels               |
| -------------- | ------------------------- | -------------------- |
| `current`      | Current in Ampere         | `address`            |
| `voltage`      | Voltage in Volt           | `address`            |
| `power`        | Power in Watt             | `address`            |
| `power_factor` | Power factor (0.0 to 1.0) | `address`            |
| `energy`       | Energy in kilowatt-hours  | `address`            |
| `temperature`  | Temperature in Celsius    | `address`, `channel` |
| `humidity`     | Humidity in percent       | `address`, `channel` |

## API Endpoint

### `/pdu`

**Method:** `GET`
**Query Parameters:**

* `target`: IP address or hostname of the PDU
* `authorization`: Basic auth string in the form `username:password` (will be Base64 encoded internally)

#### Example:

```
GET /pdu?target=192.168.1.1&authorization=username:password
```

#### Example (Manual Test):

```
http://localhost:9117/pdu?target=192.168.1.1&authorization=username:password
```

### `/api/v1/rack_names`

**Method:** `GET`
**Description:** Returns a list of rack names extracted from each PDU address block.

**Query Parameters:**

* `target`: IP address or hostname of the PDU
* `authorization`: Basic auth string in the form `username:password` (will be Base64 encoded internally)

#### Example:

```
GET /api/v1/rack_names?target=192.168.1.1&authorization=username:password
```
#### Example (Manual Test):

```
http://localhost:9117/api/v1/rack_names?target=192.168.1.1&authorization=username:password
```

#### Example Response (JSON):

```
{
  "rack_names": {
    "rack_1": "# 1 Rack A",
    "rack_2": "# 2 Rack B",
    "rack_3": "# 3 Rack C",
    ...
    "rack_30": "# 30 Rack AD",
    "rack_31": "# 31 Rack AE",
    "rack_32": "# 32 Rack AF"
  }
}
```

## Prometheus Integration

### Sample Scrape Config:

```yaml
scrape_configs:
  - job_name: 'pdu'
    metrics_path: /pdu
    static_configs:
      - targets:
        - 192.168.0.1
        labels:
          authorization: ["username:password"]
    relabel_configs:
      - source_labels: [authorization]
        target_label: __param_authorization
      - source_labels: [__address__]
        target_label: __param_target
      - source_labels: [__param_target]
        target_label: instance
      - target_label: __address__
        replacement: 127.0.0.1:9117  # Address of the PDU Exporter container or host
```
## Docker Usage

You have two options for running the PDU Exporter using Docker:

### Option 1: Build the Docker Image Locally

From the root directory of the project (where the `Dockerfile` is located), run the following command to build the image:

```bash
docker build -t pdu_exporter:latest .
```

Then, run the container:

```bash
docker run -d \
  --name pdu_exporter \
  -p 9117:9117 \
  pdu_exporter:latest
```

This will expose the exporter at `http://localhost:9117/pdu`.

### Option 2: Use Prebuilt Image from Docker Hub

You can pull the prebuilt image directly from Docker Hub:

```bash
docker pull rlggyp/pdu_exporter:latest
```

Then run the container:

```bash
docker run -d \
  --name pdu_exporter \
  -p 9117:9117 \
  rlggyp/pdu_exporter:latest
```

This achieves the same result and avoids the need to build the image manually.

## Error Handling

* Returns **400 Bad Request** if `target` or `authorization` is missing
* Returns **404 Not Found** if TCP connection to the PDU fails
* Returns **422 Unprocessable Entity** if response structure is invalid
* Returns **500 Internal Server Error** for I/O or parsing errors

## Limitations

* Assumes the PDU `/status.cgi` response has exactly 2016 elements
* Metrics parsing is tightly coupled with this structure
* Only supports plain TCP and HTTP (no TLS, no SNMP)

## License
This project is licensed under the [MIT License](LICENSE). See the LICENSE file for details.
