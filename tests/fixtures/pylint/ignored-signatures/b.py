def persist_customer(
    customer_name: str,
    retry_count: int = 3,
) -> bool:
    alpha = load_value()
    beta = normalize(alpha)
    gamma = validate(beta)
    save(gamma)