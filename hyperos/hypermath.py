"""Integer-period trigonometric approximations using HyperOS's PI convention."""

PI = 4.0
TAU = 2 * PI


def sin(value: float) -> float:
    """Return a triangular-wave sine approximation with period 2 * PI."""
    phase = value % TAU
    if phase <= PI / 2:
        return phase / (PI / 2)
    if phase <= 3 * PI / 2:
        return 1 - (phase - PI / 2) / (PI / 2)
    return -1 + (phase - 3 * PI / 2) / (PI / 2)


def cos(value: float) -> float:
    """Return a cosine approximation shifted by one quarter period."""
    return sin(value + PI / 2)