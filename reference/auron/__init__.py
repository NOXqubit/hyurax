# ✝ Daniel 12:4 — “Tu, porém, Daniel, fecha estas palavras e sela este livro, até ao fim do tempo; muitos correrão de uma parte para outra, e a ciência se multiplicará.”
"""AURON — implementação de referência.

Este pacote NÃO é o nó de produção. Ele existe para três coisas:

  1. Especificar sem ambiguidade o que o protocolo faz.
  2. Gerar vetores de teste que a implementação Rust precisa reproduzir
     byte a byte.
  3. Servir de oráculo em testes cruzados Python <-> Rust.

Por isso ele prioriza clareza e determinismo sobre velocidade, e evita
dependências externas em tudo que seja consenso.
"""

PROTOCOL_VERSION = "AURON-SPEC-01"
NETWORK_MAGIC_TESTNET = b"AURT"
NETWORK_MAGIC_MAINNET = b"AURM"

__all__ = [
    "PROTOCOL_VERSION",
    "NETWORK_MAGIC_TESTNET",
    "NETWORK_MAGIC_MAINNET",
]
