alpha = load_value()
# Normalize the incoming value.
beta = normalize(alpha)
gamma = validate(beta)
# Persist only validated data.
save(gamma)