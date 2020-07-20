#!/usr/bin/env bash
maturin build && \
  pip install --force-reinstall ../target/wheels/exogress_py-0.1.0-cp37-cp37m-macosx_10_7_x86_64.whl \
  && cd example-py/ && \
  python main.py
