#!/bin/sh
set -eu
migrate
exec voidreg-api
