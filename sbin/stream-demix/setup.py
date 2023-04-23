

#!/usr/bin/env python

from setuptools import setup, find_packages

setup(
    name='streamdeemix',
    version='1.0',
    packages=find_packages(),
    python_requires='>=3.7',
    install_requires=["deezer-py>=3.6.6"],

    entry_points={
        "console_scripts": [
            "deemixstream=deemixstream.__main__:stream",
        ]
    },
)
