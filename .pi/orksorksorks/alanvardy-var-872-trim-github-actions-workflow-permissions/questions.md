# Research Questions

## Context

The codebase contains multiple GitHub Actions workflows with varying permission configurations. The ci.yml workflow uses repository-level permissions with some job-level overrides already in place. The fly-deploy.yml workflow has no explicit permissions block. The dependabot_auto_merge.yml workflow has repository-level permissions for pull-requests and contents. Additional workflows (ci-secure.yml, rust-version-bump.yml) exist with different permission patterns.

## Questions

1. How does the existing job-level permissions pattern work in ci.yml, and what is the exact syntax and scope of the css-drift job-level override at line 131-132?

2. What permissions are actually required by the codecov-action upload steps in ci.yml, and is the current repository-level permission set (contents: read, pull-requests: write, actions: write) minimal or does it include unnecessary scopes?

3. Why does fly-deploy.yml have no explicit permissions block, and what permissions are implicitly granted through the default GITHUB_TOKEN behavior when no permissions key is specified?

4. What is the minimum set of permissions required for dependabot_auto_merge.yml to function correctly, and can the current repository-level permissions (pull-requests: write, contents: write) be reduced to job-level or further trimmed?

5. How do the permission requirements differ between ci-secure.yml's workflow-level permissions and its clippy-analyze job-level override, and what pattern does this establish for security-sensitive workflows?

6. What security best practices exist in the codebase for GitHub Actions workflows, including action pinning strategies, secret usage patterns, and permission minimization conventions?

7. How does the rust-version-bump.yml workflow's permission configuration compare to the other workflows, and what implications does its use of a private token (MYTOKEN) have for permission requirements?
