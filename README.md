# PDU Exporter

The **PDU Exporter** is a lightweight custom Prometheus exporter designed to collect metrics from Power Distribution Unit (PDU) devices that expose raw status data via the `/status.cgi` endpoint over HTTP, transforming this data into Prometheus-compatible metrics.

## Features

* Connects directly to the PDU via raw TCP (port 80)
* Sends a manual HTTP GET request
* Supports basic authentication
* Exposes metrics such as current, voltage, power, energy, temperature, humidity, and sensor existence
* Dockerized for easy deployment

## Exported Metrics

| Metric Name    | Description                      | Labels               |
| -------------- | -------------------------------- | -------------------- |
| `current`      | Current in Amperes               | `address`            |
| `voltage`      | Voltage in Volts                 | `address`            |
| `power`        | Power in Watts                   | `address`            |
| `power_factor` | Power factor (0.0 to 1.0)        | `address`            |
| `energy`       | Energy in kilowatt-hours         | `address`            |
| `temperature`  | Temperature in Celsius           | `address`, `channel` |
| `humidity`     | Humidity in percent              | `address`, `channel` |
| `sensor_exists`| Sensor existence (1.0 or 0.0)    | `type`               |

## API Endpoints

### `/-/reload`

**Method:** `POST, PUT`
**Description:** Reloads the configuration without restarting the process.

#### Example:

```
POST /-/reload
PUT /-/reload
```
You can also reload configuration by sending the SIGHUP signal to the `pdu_exporter` process ID using the command below:
```bash
kill -s SIGHUP $(pidof pdu_exporter)
```

### `/pdu`

**Method:** `GET`
**Query Parameters:**

* `target`: IP address or hostname of the PDU.

#### Example:

```
GET /pdu?target=192.168.1.1
```

### `/api/rack-names`

**Method:** `GET`
**Description:** Returns a list of rack names extracted from each PDU address block.

**Query Parameters:**

* `target`: IP address or hostname of the PDU.

#### Example:

```
GET /api/rack-names?target=192.168.1.1
```

#### Example Response (JSON):

```
{
  "rack_names": {
    "rack_1": "# 1 Rack A",
    "rack_2": "# 2 Rack B",
    ...
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
        - 192.168.1.1
    relabel_configs:
      - source_labels: [__address__]
        target_label: __param_target
      - source_labels: [__param_target]
        target_label: instance
      - target_label: __address__
        replacement: 127.0.0.1:9117 # Address of the PDU Exporter container or host
    basic_auth: # Sets the `Authorization` header on every request
      username: user # Configured username
      password: pass # Configured password
```

## PDU Exporter Configuration

### Example `pdu_exporter.yaml`

```yaml
# How long until a scrape request times out.
scrape_configs:
  scrape_timeout: 5s

# Usernames and hashed passwords that have full access to the web server via basic authentication.
# If empty, no basic authentication is required. Passwords are hashed with bcrypt.
basic_auth_users:
  user: $2a$12$eId/v3HxJFjZWCkeTV/.VeUg5Qie66aPumgGlzELcy2ndCxpo5fV6
  rlggyp: $2a$12$fKG1d9B5d7JDa78s7XBHDu/YD.46VK.t3W8BnujOwErrNCvRB9vsS
```

### Example `log4rs.yaml`

```yaml
refresh_rate: 30 seconds

appenders:
  rolling_file:
    kind: rolling_file
    path: "/etc/pdu_exporter/logs/app.log"
    policy:
      kind: compound
      trigger:
        kind: size
        limit: 1 mb
      roller:
        kind: fixed_window
        pattern: "/etc/pdu_exporter/logs/app-{}.log"
        base: 1
        count: 7
    encoder:
      pattern: "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}"

root:
  level: info
  appenders:
    - rolling_file
```

## Docker Usage

### Docker Compose Example

Below is a sample `docker-compose.yaml`:

```yaml
services:
  pdu_exporter:
    image: rlggyp/pdu_exporter:latest
    container_name: pdu_exporter
    user: 1000:1000
    pull_policy: always
    environment:
      - CONFIG_FILE=/etc/pdu_exporter/configs/pdu_exporter.yaml
      - LOG_CONFIG_FILE=/etc/pdu_exporter/configs/log4rs.yaml
    ports:
      - 9117:9117
    volumes:
      - ./configs:/etc/pdu_exporter/configs
      - ./logs:/etc/pdu_exporter/logs
    restart: unless-stopped
```

**Directory structure example:**
```
.
├── configs
│   ├── pdu_exporter.yaml
│   └── log4rs.yaml
├── logs
└── docker-compose.yaml
```

- Place your `pdu_exporter.yaml` and `log4rs.yaml` in the `configs` directory.
- Logs will be written to the `logs` directory.

---

### Running with Docker Compose

You have two options to run the PDU Exporter:

#### Option 1: Build the Docker Image Locally

1. Build the image:

    ```bash
    docker build -t pdu_exporter:latest .
    ```

2. Edit your `docker-compose.yaml` and change the `image` line to use your local image:

    ```yaml
    image: pdu_exporter:latest
    ```

3. Start the service:

    ```bash
    docker compose up -d
    ```

#### Option 2: Use the Prebuilt Image from Docker Hub

No need to build anything. Just use the provided `docker-compose.yaml` (with `image: rlggyp/pdu_exporter:latest`) and run:

```bash
docker compose up -d
```

## Error Handling

* Returns **400 Bad Request** if `target` is missing.
* Returns **401 Unauthorized** if authentication fails.
* Returns **404 Not Found** if the TCP connection to the PDU fails.
* Returns **408 Request Timeout** if the connection to `target` times out.
* Returns **422 Unprocessable Entity** if the response structure is invalid.
* Returns **500 Internal Server Error** for I/O or parsing errors.

## Limitations

* Assumes the PDU `/status.cgi` response contains exactly 2016 elements.
* Metrics parsing is tightly coupled with this structure.
* Only supports plain TCP and HTTP (no TLS, no SNMP).

## License

This project is licensed under the [MIT License](LICENSE). See the LICENSE file for details.
