# Security Notes

This template is designed for personal local automation, not blind unrestricted autonomy.

## Recommended safety baseline

- Run the agent on a dedicated machine or user account.
- Use separate branches for every run.
- Keep secrets outside the repository.
- Never allow `git push` in unattended mode.
- Review every commit before merging.
- Keep `.agent.env` and `.claude/settings.local.json` out of git.
- Use project-specific credentials with minimal scope when credentials are unavoidable.

## Files intentionally ignored

- `.agent.env`
- `.env`
- `.env.*`
- `.claude/settings.local.json`
- runtime logs

## Permission guidance

Start with conservative permissions and loosen them only after successful manual runs.

Avoid unrestricted permission bypass unless the agent is running in an isolated disposable environment.
