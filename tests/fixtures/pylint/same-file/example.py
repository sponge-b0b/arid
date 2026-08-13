def process_account():
    alpha = load_value()
    beta = normalize(alpha)
    gamma = validate(beta)
    save(gamma)


def process_customer():
    alpha = load_value()
    beta = normalize(alpha)
    gamma = validate(beta)
    save(gamma)