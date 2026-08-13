def process():
    """Load and persist a validated value."""
    alpha = load_value()
    beta = normalize(alpha)
    gamma = validate(beta)
    save(gamma)