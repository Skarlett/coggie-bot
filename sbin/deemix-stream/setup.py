#!/usr/bin/env python
from setuptools import setup, find_packages
setup(
    name='deemix-stream',
    version='1.0',
    packages=find_packages(),
    python_requires='>=3.7',
    install_requires=["deezer-py>=1.3.0"],
    scripts = ["deemix-stream"]
)
