# Contributing to Plott

Thanks for your interest in contributing to Plott!

## Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes
4. Run tests: `npm test`
5. Commit using conventional commits: `feat: add new feature`
6. Push and open a pull request

## Development Setup

```bash
npm install
docker compose up -d postgres redis
npm run dev
```

## Running Tests

```bash
npm test
npm run test:coverage
```

## Code Style

- TypeScript strict mode
- ESLint for linting
- Conventional commits for commit messages

## Reporting Issues

Please use GitHub Issues to report bugs or request features.
