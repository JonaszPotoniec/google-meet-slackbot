#!/bin/bash

# Security Validation Script for Slack Meet Bot
# Run this script to validate security configurations

set -e

echo "🔍 SECURITY VALIDATION CHECKLIST"
echo "================================="

Check 1: Verify no real credentials in files
echo "1. Checking for exposed credentials..."
if grep -r "GOCSPX-" . --exclude-dir=.git --exclude="*.md" --exclude=".env" 2>/dev/null; then
    echo "❌ CRITICAL: Google OAuth secret found in files!"
    exit 1
fi

if grep -r "07bf38ee" . --exclude-dir=.git --exclude="*.md" --exclude=".env" 2>/dev/null; then
    echo "❌ CRITICAL: Slack signing secret found in files!"
    exit 1
fi

echo "✅ No hardcoded credentials found"

# Check 2: Verify .env is gitignored
echo "2. Checking .gitignore configuration..."
if grep -q "^\.env$" .gitignore; then
    echo "✅ .env is properly gitignored"
else
    echo "❌ WARNING: .env should be in .gitignore"
fi

# Check 3: Check for unsafe Rust patterns
echo "3. Scanning for unsafe Rust patterns..."
if grep -r "unwrap()" src/ 2>/dev/null | grep -v test; then
    echo "⚠️ WARNING: Found unwrap() calls that could panic"
fi

if grep -r "expect(" src/ 2>/dev/null | grep -v test; then
    echo "⚠️ WARNING: Found expect() calls that could panic"
fi

if grep -r "unsafe" src/ 2>/dev/null; then
    echo "⚠️ WARNING: Found unsafe code blocks"
fi

# Check 4: Docker security configuration
echo "4. Validating Docker configuration..."
if [ -f "docker-compose.prod.yml" ]; then
    if grep -q "read_only: true" docker-compose.prod.yml && grep -q "./data:/app/data" docker-compose.prod.yml; then
        echo "❌ CRITICAL: Docker production config has read-only conflict with database writes"
    else
        echo "✅ Docker production configuration looks secure"
    fi
fi

# Check 5: Database file permissions
echo "5. Checking database file security..."
if [ -f "app.db" ]; then
    DB_PERMS=$(stat -f "%A" app.db 2>/dev/null || stat -c "%a" app.db 2>/dev/null || echo "unknown")
    if [ "$DB_PERMS" = "644" ] || [ "$DB_PERMS" = "600" ]; then
        echo "✅ Database file permissions are secure ($DB_PERMS)"
    else
        echo "⚠️ WARNING: Database file permissions are too open ($DB_PERMS)"
    fi
fi

# Check 6: Dependencies security scan
echo "6. Checking for security vulnerabilities in dependencies..."
if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit
else
    echo "⚠️ INFO: Install cargo-audit for dependency vulnerability scanning:"
    echo "   cargo install cargo-audit"
fi

# Check 7: Test presence
echo "7. Verifying security tests exist..."
if grep -r "test.*security\|test.*verify\|test.*signature" src/ >/dev/null 2>&1; then
    echo "✅ Security tests found"
else
    echo "⚠️ WARNING: No security-specific tests found"
fi

# Check 8: Required environment variables
echo "8. Checking required environment variables..."
REQUIRED_VARS=(
    "SLACK_SIGNING_SECRET"
    "GOOGLE_CLIENT_ID" 
    "GOOGLE_CLIENT_SECRET"
    "GOOGLE_REDIRECT_URI"
    "DATABASE_URL"
)

for var in "${REQUIRED_VARS[@]}"; do
    if [ -z "${!var}" ]; then
        echo "⚠️ WARNING: Environment variable $var is not set"
    fi
done

# Check 9: File structure security
echo "9. Validating secure file structure..."
if [ -d "data" ] && [ "$(ls -A data)" ]; then
    echo "⚠️ INFO: Data directory contains files"
fi

if [ -f ".env" ]; then
    echo "⚠️ WARNING: .env file exists - ensure it contains only example values"
fi

# Security scoring
echo ""
echo "🔒 SECURITY VALIDATION COMPLETE"
echo "================================"
echo ""
echo "📋 CRITICAL ACTIONS REQUIRED:"
echo "1. Rotate all exposed credentials immediately"
echo "2. Implement token encryption in database"  
echo "3. Add input validation and rate limiting"
echo "4. Fix Docker production configuration conflicts"
echo "5. Complete security testing before production"
echo ""
echo "Run this script regularly to validate security posture!"
