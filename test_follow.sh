#!/bin/bash
USER2_ID="ea6dde85-7fdc-4f2a-9cdb-3fc727feb036"
LOGIN_RESPONSE=$(curl -s -X POST http://localhost:8000/api/auth/sign-in -H "Content-Type: application/json" -d '{"email": "test@example.com", "password": "password123"}')
TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"token":"[^"]*"' | sed 's/"token":"//;s/"$//')

echo "Token: ${TOKEN:0:30}..."
echo ""

echo "=== Testing Profile ==="
RESULT=$(curl -s "http://localhost:8000/api/user/$USER2_ID/profile" -H "Authorization: Bearer $TOKEN")
echo "$RESULT"
echo ""

echo "=== Testing Followers ==="
RESULT=$(curl -s "http://localhost:8000/api/user/$USER2_ID/followers")
echo "$RESULT"
echo ""

echo "=== Testing Follow (POST) ==="
RESULT=$(curl -s -X POST "http://localhost:8000/api/user/$USER2_ID/follow" -H "Authorization: Bearer $TOKEN")
echo "$RESULT"
echo ""
