function greet(name) {
  return `Hello, ${name}!`;
}

module.exports = Object.assign(greet, { extra: "extra property" });
