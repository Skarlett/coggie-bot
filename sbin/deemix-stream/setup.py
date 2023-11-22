from setuptools import setup, find_packages

setup(
    name='deemix_stream',
    version='0.0.3',
    python_requires='>=3.7',
    # Modules to import from other scripts:
    packages=find_packages(),
    install_requires=["click", "requests", "deemix>=3.6.6", "spotipy>=2.16.1"],
    # Executables
    scripts=["deemix-stream", "deemix-metadata", "spotify-recommend"],
)
