# pithon

Python helpers for Pi-centric experimentation.

## Install

```bash
pip install -e .
```

## Example

```python
from pithon.phase import wrap_tau, coherence_gate

print(wrap_tau(-1.0))
print(coherence_gate(0.1, 0.2, 0.5))
```
