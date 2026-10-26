# Contract: Measurement Files

Written by `tools/analyze.py` into `specs/20261026-154808-stack-spike/measurements/`.
Fields are defined in [data-model.md](../data-model.md).

## video-samples.csv

```csv
clip,path,variant,mode,platform,flash_index,delay_ms
```

## audio-samples.csv

```csv
clip,platform,output,minute,beep_index,delay_ms
```

## summary.md

Markdown tables generated from the CSV files: one video table per platform (rows: path and
variant; columns per mode: n, median, p95, dropped frames) and one audio table (median, p95, A/V
offset, drift). The findings report copies or links these tables.

Clips (`*.mp4`) stay in a local folder and are not committed; the CSV keeps the file name so a result
can be traced back to its clip.
