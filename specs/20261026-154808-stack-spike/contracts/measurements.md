# Contract: Measurement Files

Written by `tools/latency/analyze.py` into the folder given by `--measurements`; for this spike
`specs/20261026-154808-stack-spike/measurements/`.
Fields are defined in [data-model.md](../data-model.md).

## video-samples.csv

```csv
clip,path,variant,mode,platform,flash_index,delay_ms
```

## audio-samples.csv

```csv
clip,platform,output,minute,beep_index,delay_ms
```

## pipeline-stats.csv

Written by the prototypes with `--record` (one row per second of a run). `--stats` only prints, so
test runs never mix with measurements.

```csv
run,path,variant,mode,platform,second,rendered,dropped,pipeline_latency_ms
```

## summary.md

Markdown tables generated from the CSV files: one video table per platform and mode (rows: path
and variant; columns: n, median, p95, min, max, dropped frames; every path is listed, "n/a" when
not measured) and one audio table (per platform, output and minute: n, median, p95, A/V offset,
drift). The findings report copies or links these tables.

Clips (`*.mp4`) stay in a local folder and are not committed; the CSV keeps the file name so a result
can be traced back to its clip.
