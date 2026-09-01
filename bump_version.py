import re

with open('Cargo.toml', 'r') as f:
    content = f.read()

def bump_match(m):
    major, minor, patch = map(int, m.group(1).split('.'))
    patch += 1
    new_ver = f"{major}.{minor}.{patch}"
    print(f"Bumping version from {m.group(1)} to {new_ver}")
    return f'version = "{new_ver}"'

new_content = re.sub(r'version\s*=\s*"(\d+\.\d+\.\d+)"', bump_match, content, count=1)

with open('Cargo.toml', 'w') as f:
    f.write(new_content)
