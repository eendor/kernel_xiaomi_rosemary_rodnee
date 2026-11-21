#!/system/bin/sh
if ! pgrep -x rodnee-tuner >/dev/null 2>&1; then
  nohup /data/adb/rodnee/rodnee-tuner >/dev/null 2>&1 &
fi
