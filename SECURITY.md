# Security policy

Please report potential vulnerabilities privately through GitHub's security advisory feature. Do not open a public
issue for an unpatched vulnerability.

Supported releases receive fixes on the latest minor line. Regular expressions use Rust's finite-automata engine, and
SQL values are bound or escaped; callers remain responsible for supplying trusted SQL column expressions.
