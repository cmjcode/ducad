#!/usr/bin/env bash
# ==============================================================================
# DuCAD API & Backend Integration Test Suite
# Tests DuCAD Cloud, Authentication, and Touch Design Sync Endpoints
# ==============================================================================

set -euo pipefail

# Text styling
BOLD="\033[1m"
GREEN="\033[0;32m"
RED="\033[0;31m"
BLUE="\033[0;34m"
YELLOW="\033[0;33m"
NC="\033[0m" # No Color

PASSED_COUNT=0
FAILED_COUNT=0
TOTAL_COUNT=0

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    PASSED_COUNT=$((PASSED_COUNT + 1))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    FAILED_COUNT=$((FAILED_COUNT + 1))
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Determine Port
TEST_PORT=${TEST_API_PORT:-38491}
SERVER_HOST="127.0.0.1"
BASE_URL="http://${SERVER_HOST}:${TEST_PORT}"

# Temporary files
TMP_DIR=$(mktemp -d)
SERVER_LOG="${TMP_DIR}/mock_server.log"
SERVER_PID_FILE="${TMP_DIR}/server.pid"

cleanup() {
    if [[ -f "${SERVER_PID_FILE}" ]]; then
        PID=$(cat "${SERVER_PID_FILE}" 2>/dev/null || true)
        if [[ -n "${PID}" ]] && kill -0 "${PID}" 2>/dev/null; then
            log_info "Stopping mock test server (PID: ${PID})..."
            kill "${PID}" 2>/dev/null || true
            wait "${PID}" 2>/dev/null || true
        fi
    fi
    rm -rf "${TMP_DIR}"
}
trap cleanup EXIT INT TERM

# Start lightweight mock server for DuCAD Cloud endpoints
start_mock_server() {
    log_info "Starting DuCAD Cloud mock API server on ${BASE_URL}..."

    python3 - <<EOF > "${SERVER_LOG}" 2>&1 &
import http.server
import json
import urllib.parse

PORT = ${TEST_PORT}
stored_touch_config = {
    "mode": "PencilAndFinger",
    "palm_rejection": True,
    "touch_target_size": 44.0
}

class DuCadMockHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Keep stdout clean
        pass

    def _send_json(self, status, payload):
        body = json.dumps(payload).encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        query = urllib.parse.parse_qs(parsed.query)

        if path == "/api/v1/health":
            self._send_json(200, {
                "status": "ok",
                "version": "0.1.0",
                "service": "ducad-cloud",
                "features": ["oauth", "cloud-sync", "ipad-touch-design"]
            })
        elif path == "/api/v1/auth/login/google":
            client = query.get("client", [""])[0]
            port = query.get("port", [""])[0]
            self._send_json(200, {
                "provider": "google",
                "client": client,
                "callback_port": port,
                "status": "ready"
            })
        elif path == "/api/v1/auth/login/github":
            client = query.get("client", [""])[0]
            port = query.get("port", [""])[0]
            self._send_json(200, {
                "provider": "github",
                "client": client,
                "callback_port": port,
                "status": "ready"
            })
        elif path == "/api/v1/user/profile":
            auth_hdr = self.headers.get("Authorization", "")
            if not auth_hdr.startswith("Bearer "):
                self._send_json(401, {"error": "Unauthorized"})
                return
            self._send_json(200, {
                "id": "usr_ipad_tester_01",
                "email": "designer@ducad.org",
                "display_name": "DuCAD iPad Designer",
                "license_tier": "Pro"
            })
        elif path == "/api/v1/sync/touch-config":
            global stored_touch_config
            self._send_json(200, {
                "status": "success",
                "config": stored_touch_config
            })
        else:
            self._send_json(404, {"error": "Endpoint not found"})

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        if path == "/api/v1/sync/touch-config":
            global stored_touch_config
            content_len = int(self.headers.get('Content-Length', 0))
            post_body = self.rfile.read(content_len)
            try:
                data = json.loads(post_body.decode('utf-8'))
                stored_touch_config.update(data)
                self._send_json(200, {
                    "status": "synced",
                    "updated_config": stored_touch_config
                })
            except Exception as e:
                self._send_json(400, {"error": str(e)})
        elif path == "/api/v1/auth/callback":
            content_len = int(self.headers.get('Content-Length', 0))
            post_body = self.rfile.read(content_len)
            try:
                token_data = json.loads(post_body.decode('utf-8'))
                self._send_json(200, {
                    "status": "authenticated",
                    "access_token": token_data.get("access_token", ""),
                    "user": token_data.get("user", {})
                })
            except Exception as e:
                self._send_json(400, {"error": str(e)})
        else:
            self._send_json(404, {"error": "Endpoint not found"})

server = http.server.HTTPServer(("${SERVER_HOST}", PORT), DuCadMockHandler)
server.serve_forever()
EOF
    SERVER_PID=$!
    echo "${SERVER_PID}" > "${SERVER_PID_FILE}"

    # Wait for server readiness
    local retries=20
    while ! curl -s -f "${BASE_URL}/api/v1/health" > /dev/null 2>&1; do
        sleep 0.1
        retries=$((retries - 1))
        if [[ ${retries} -le 0 ]]; then
            log_fail "Mock server failed to start within timeout. Check ${SERVER_LOG}"
            exit 1
        fi
    done
    log_info "Mock server started successfully (PID: ${SERVER_PID})."
}

# Helper to assert HTTP response status and body content
assert_request() {
    local test_name="$1"
    local method="$2"
    local path="$3"
    local expected_status="$4"
    local req_data="${5:-}"
    local auth_header="${6:-}"
    local body_pattern="${7:-}"

    local curl_cmd=(curl -s -w "\n%{http_code}" -X "${method}")
    if [[ -n "${auth_header}" ]]; then
        curl_cmd+=(-H "Authorization: ${auth_header}")
    fi
    if [[ -n "${req_data}" ]]; then
        curl_cmd+=(-H "Content-Type: application/json" -d "${req_data}")
    fi
    curl_cmd+=("${BASE_URL}${path}")

    local response
    response=$("${curl_cmd[@]}")
    local status_code
    status_code=$(echo "${response}" | tail -n1)
    local body
    body=$(echo "${response}" | sed '$d')

    if [[ "${status_code}" != "${expected_status}" ]]; then
        log_fail "${test_name} - Expected HTTP ${expected_status}, got ${status_code}. Body: ${body}"
        return
    fi

    if [[ -n "${body_pattern}" ]]; then
        if echo "${body}" | grep -q "${body_pattern}"; then
            log_pass "${test_name} (HTTP ${status_code}, Pattern '${body_pattern}' matched)"
        else
            log_fail "${test_name} - Body did not match pattern '${body_pattern}'. Body: ${body}"
        fi
    else
        log_pass "${test_name} (HTTP ${status_code})"
    fi
}

# Main Execution
echo -e "${BOLD}======================================================${NC}"
echo -e "${BOLD}        DuCAD Cloud & Touch API Test Suite            ${NC}"
echo -e "${BOLD}======================================================${NC}"

start_mock_server

echo ""
log_info "Running API Endpoint Tests..."

# Test 1: Health Check Endpoint
assert_request \
    "Test 1: Health Check" \
    "GET" \
    "/api/v1/health" \
    "200" \
    "" \
    "" \
    "\"status\": \"ok\""

# Test 2: OAuth Provider Google
assert_request \
    "Test 2: OAuth Login Flow (Google)" \
    "GET" \
    "/api/v1/auth/login/google?client=ducad&port=12345" \
    "200" \
    "" \
    "" \
    "\"provider\": \"google\""

# Test 3: OAuth Provider GitHub
assert_request \
    "Test 3: OAuth Login Flow (GitHub)" \
    "GET" \
    "/api/v1/auth/login/github?client=ducad&port=12345" \
    "200" \
    "" \
    "" \
    "\"provider\": \"github\""

# Test 4: User Profile API - Unauthorized without Bearer token
assert_request \
    "Test 4: User Profile (Unauthorized check)" \
    "GET" \
    "/api/v1/user/profile" \
    "401" \
    "" \
    "" \
    "\"error\": \"Unauthorized\""

# Test 5: User Profile API - Authorized with Bearer token
assert_request \
    "Test 5: User Profile (Authorized with token)" \
    "GET" \
    "/api/v1/user/profile" \
    "200" \
    "" \
    "Bearer test_token_ducad_secret" \
    "\"display_name\": \"DuCAD iPad Designer\""

# Test 6: Sync Touch Design Config (iPad Feature)
TOUCH_PAYLOAD='{"mode": "PencilOnly", "palm_rejection": true, "touch_target_size": 44.0}'
assert_request \
    "Test 6: Sync Touch Design Configuration (Apple Pencil Only)" \
    "POST" \
    "/api/v1/sync/touch-config" \
    "200" \
    "${TOUCH_PAYLOAD}" \
    "" \
    "\"status\": \"synced\""

# Test 7: Verify Synced Touch Configuration Retrieval
assert_request \
    "Test 7: Fetch Synced Touch Design Configuration" \
    "GET" \
    "/api/v1/sync/touch-config" \
    "200" \
    "" \
    "" \
    "\"mode\": \"PencilOnly\""

# Test 8: Loopback Callback Token Verification
CALLBACK_PAYLOAD='{"access_token": "tok_xyz_123", "user": {"id": "usr_01", "email": "designer@ducad.org"}}'
assert_request \
    "Test 8: Auth Loopback Callback Handler" \
    "POST" \
    "/api/v1/auth/callback" \
    "200" \
    "${CALLBACK_PAYLOAD}" \
    "" \
    "\"access_token\": \"tok_xyz_123\""

# Test 9: 404 Route Handling
assert_request \
    "Test 9: Invalid Endpoint returns 404" \
    "GET" \
    "/api/v1/nonexistent-endpoint" \
    "404" \
    "" \
    "" \
    "\"error\": \"Endpoint not found\""

echo ""
echo -e "${BOLD}======================================================${NC}"
echo -e "${BOLD}                   Test Summary                       ${NC}"
echo -e "${BOLD}======================================================${NC}"
echo -e "Total Tests : ${TOTAL_COUNT}"
echo -e "Passed      : ${GREEN}${PASSED_COUNT}${NC}"
echo -e "Failed      : ${RED}${FAILED_COUNT}${NC}"

if [[ ${FAILED_COUNT} -gt 0 ]]; then
    echo -e "${RED}${BOLD}❌ Test Suite FAILED!${NC}"
    exit 1
else
    echo -e "${GREEN}${BOLD}✅ All API tests PASSED successfully!${NC}"
    exit 0
fi
