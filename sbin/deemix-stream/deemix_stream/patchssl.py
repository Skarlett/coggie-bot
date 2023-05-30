#!/usr/bin/env python
#######################
# Patch for SSL error #
#######################
from os import environ
environ['CURL_CA_BUNDLE'] = ""
environ['REQUESTS_CA_BUNDLE'] = ""