#!/bin/bash

PROCESS_NAME="${1:-crash-cache}"
INTERVAL="${2:-0.5}"

PID=$(pgrep -f "$PROCESS_NAME" | head -1)

if [ -z "$PID" ]; then
    echo "Process '$PROCESS_NAME' not found"
    exit 1
fi

echo "=============================================="
echo "    MONITORING: $PROCESS_NAME (PID: $PID)"
echo "=============================================="
echo ""
printf "%-10s %-10s %-10s %-10s %-10s\n" "TIME" "CPU%" "MEM%" "RSS(MB)" "THREADS"
echo "------------------------------------------------------"

while true; do
    if [ -d "/proc/$PID" ]; then
        STATS=$(ps -p $PID -o %cpu,%mem,rss,nlwp --no-headers 2>/dev/null)
        if [ -n "$STATS" ]; then
            CPU=$(echo $STATS | awk '{print $1}')
            MEM=$(echo $STATS | awk '{print $2}')
            RSS=$(echo $STATS | awk '{printf "%.1f", $3/1024}')
            THREADS=$(echo $STATS | awk '{print $4}')
            TIME=$(date +%H:%M:%S)
            printf "%-10s %-10s %-10s %-10s %-10s\n" "$TIME" "$CPU" "$MEM" "$RSS" "$THREADS"
        fi
    else
        echo "Process terminated"
        exit 0
    fi
    sleep $INTERVAL
done
