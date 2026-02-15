module.exports = {
  preset: "ts-jest",
  testEnvironment: "node",
  roots: ["<rootDir>/src"],
  testMatch: ["**/*.test.ts"],
  moduleFileExtensions: ["ts", "js", "json"],
};
# add abort controller for request timeouts [3.15]
# add pattern result display formatting [4.15]
# wip: refactor prediction window logic [7.15]
