def process_account(account_id: int) -> None:
    alpha = load_value()
    beta = normalize(alpha)
    gamma = validate(beta)
    save(gamma)