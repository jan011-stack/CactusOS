"""Experimental HyperOS mathematical and binary-vector utilities."""

from .hypermath import PI, TAU, cos, sin
from .terminal_bridge import BinaryVector, encode_command, process_command, transform

__all__ = [
    "PI",
    "TAU",
    "BinaryVector",
    "cos",
    "encode_command",
    "process_command",
    "sin",
    "transform",
]