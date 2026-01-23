#!/bin/bash

URL="${1:-http://localhost:3000/api/1/store/}"
PAYLOAD="${2:-test_payloads/report_full.json}"
REQUESTS="${3:-100}"
CONCURRENCY="${4:-10}"
CONTENT_ENCODING="${5:-}"

echo "=============================================="
echo "       CRASH-CACHE LOAD TEST"
echo "=============================================="
echo "URL:          $URL"
echo "Payload:      $PAYLOAD ($(stat -c%s "$PAYLOAD") bytes)"
echo "Requests:     $REQUESTS"
echo "Concurrency:  $CONCURRENCY"
echo "Encoding:     ${CONTENT_ENCODING:-none}"
echo "=============================================="

RESULTS_DIR="/tmp/loadtest_$$"
mkdir -p "$RESULTS_DIR"

if [ -n "$CONTENT_ENCODING" ]; then
    CURL_HEADERS="-H 'Content-Type: application/json' -H 'Content-Encoding: $CONTENT_ENCODING'"
else
    CURL_HEADERS="-H 'Content-Type: application/json'"
fi

single_request() {
    local id=$1
    local start=$(date +%s%N)
    
    if [ -n "$CONTENT_ENCODING" ]; then
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
            -X POST \
            -H "Content-Type: application/json" \
            -H "Content-Encoding: $CONTENT_ENCODING" \
            --data-binary "@$PAYLOAD" \
            "$URL" 2>/dev/null)
    else
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
            -X POST \
            -H "Content-Type: application/json" \
            --data-binary "@$PAYLOAD" \
            "$URL" 2>/dev/null)
    fi
    
    local end=$(date +%s%N)
    local duration=$(( (end - start) / 1000000 ))
    
    echo "$HTTP_CODE $duration" >> "$RESULTS_DIR/results.txt"
}
export -f single_request
export URL PAYLOAD CONTENT_ENCODING RESULTS_DIR

START_TIME=$(date +%s%N)

seq 1 $REQUESTS | xargs -P $CONCURRENCY -I {} bash -c 'single_request {}'

END_TIME=$(date +%s%N)
TOTAL_TIME=$(( (END_TIME - START_TIME) / 1000000 ))

echo ""
echo "=============================================="
echo "                 RESULTS"
echo "=============================================="

if [ -f "$RESULTS_DIR/results.txt" ]; then
    TOTAL=$(wc -l < "$RESULTS_DIR/results.txt")
    SUCCESS=$(grep -c "^200 " "$RESULTS_DIR/results.txt" || echo 0)
    OVERLOAD=$(grep -c "^503 " "$RESULTS_DIR/results.txt" || echo 0)
    ERRORS=$(grep -cv "^200 \|^503 " "$RESULTS_DIR/results.txt" || echo 0)
    
    TIMES=$(awk '{print $2}' "$RESULTS_DIR/results.txt" | sort -n)
    MIN=$(echo "$TIMES" | head -1)
    MAX=$(echo "$TIMES" | tail -1)
    AVG=$(echo "$TIMES" | awk '{sum+=$1} END {printf "%.0f", sum/NR}')
    
    P50_IDX=$(( (TOTAL * 50) / 100 ))
    P95_IDX=$(( (TOTAL * 95) / 100 ))
    P99_IDX=$(( (TOTAL * 99) / 100 ))
    [ $P50_IDX -lt 1 ] && P50_IDX=1
    [ $P95_IDX -lt 1 ] && P95_IDX=1
    [ $P99_IDX -lt 1 ] && P99_IDX=1
    
    P50=$(echo "$TIMES" | sed -n "${P50_IDX}p")
    P95=$(echo "$TIMES" | sed -n "${P95_IDX}p")
    P99=$(echo "$TIMES" | sed -n "${P99_IDX}p")
    
    RPS=$(echo "scale=2; $TOTAL * 1000 / $TOTAL_TIME" | bc)
    
    echo "Total requests:     $TOTAL"
    echo "Successful (200):   $SUCCESS"
    echo "Overloaded (503):   $OVERLOAD"
    echo "Errors:             $ERRORS"
    echo ""
    echo "Total time:         ${TOTAL_TIME}ms"
    echo "Requests/sec:       $RPS"
    echo ""
    echo "Response times (ms):"
    echo "  Min:    $MIN"
    echo "  Avg:    $AVG"
    echo "  Max:    $MAX"
    echo "  P50:    $P50"
    echo "  P95:    $P95"
    echo "  P99:    $P99"
    
    echo ""
    echo "Status code distribution:"
    awk '{print $1}' "$RESULTS_DIR/results.txt" | sort | uniq -c | sort -rn
fi

rm -rf "$RESULTS_DIR"
echo "=============================================="
