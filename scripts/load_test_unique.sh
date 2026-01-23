#!/bin/bash

URL="${1:-http://localhost:3000/api/1/store/}"
REQUESTS="${2:-100}"
CONCURRENCY="${3:-10}"

echo "=============================================="
echo "  UNIQUE PAYLOADS LOAD TEST"
echo "=============================================="
echo "URL:          $URL"
echo "Requests:     $REQUESTS"
echo "Concurrency:  $CONCURRENCY"
echo "=============================================="

RESULTS_DIR="/tmp/loadtest_unique_$$"
mkdir -p "$RESULTS_DIR"

PAYLOADS=(test_payloads/report_unique_*.json)
NUM_PAYLOADS=${#PAYLOADS[@]}
echo "Available payloads: $NUM_PAYLOADS"

single_request() {
    local id=$1
    local idx=$((id % NUM_PAYLOADS))
    local payload="${PAYLOADS[$idx]}"
    local start=$(date +%s%N)
    
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -X POST \
        -H "Content-Type: application/json" \
        --data-binary "@$payload" \
        "$URL" 2>/dev/null)
    
    local end=$(date +%s%N)
    local duration=$(( (end - start) / 1000000 ))
    
    echo "$HTTP_CODE $duration" >> "$RESULTS_DIR/results.txt"
}
export -f single_request
export URL RESULTS_DIR
export PAYLOADS NUM_PAYLOADS

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
    SUCCESS=$(grep -c "^200 " "$RESULTS_DIR/results.txt" 2>/dev/null || echo 0)
    OVERLOAD=$(grep -c "^503 " "$RESULTS_DIR/results.txt" 2>/dev/null || echo 0)
    
    TIMES=$(awk '{print $2}' "$RESULTS_DIR/results.txt" | sort -n)
    AVG=$(echo "$TIMES" | awk '{sum+=$1} END {printf "%.0f", sum/NR}')
    P50_IDX=$(( (TOTAL * 50) / 100 )); [ $P50_IDX -lt 1 ] && P50_IDX=1
    P95_IDX=$(( (TOTAL * 95) / 100 )); [ $P95_IDX -lt 1 ] && P95_IDX=1
    P99_IDX=$(( (TOTAL * 99) / 100 )); [ $P99_IDX -lt 1 ] && P99_IDX=1
    
    RPS=$(echo "scale=2; $TOTAL * 1000 / $TOTAL_TIME" | bc)
    
    echo "Total:        $TOTAL"
    echo "Success:      $SUCCESS"
    echo "Overload 503: $OVERLOAD"
    echo "Total time:   ${TOTAL_TIME}ms"
    echo "Req/sec:      $RPS"
    echo "Avg latency:  ${AVG}ms"
    echo "P50:          $(echo "$TIMES" | sed -n "${P50_IDX}p")ms"
    echo "P95:          $(echo "$TIMES" | sed -n "${P95_IDX}p")ms"
    echo "P99:          $(echo "$TIMES" | sed -n "${P99_IDX}p")ms"
    echo ""
    echo "Status codes:"
    awk '{print $1}' "$RESULTS_DIR/results.txt" | sort | uniq -c | sort -rn
fi

rm -rf "$RESULTS_DIR"
