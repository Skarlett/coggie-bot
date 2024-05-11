#!/usr/bin/env python3
import deemix_stream.patchssl
from deemix.types.DownloadObjects import Single, Collection, Convertable
from deemix.plugins.spotify import Spotify
import spotipy

SpotifyClientCredentials = spotipy.oauth2.SpotifyClientCredentials
CacheFileHandler = spotipy.cache_handler.CacheFileHandler

class MockCache(dict):
    def __getitem__(self, _key):
        return {}

class SpotifyStreamer(Spotify):
    def __init__(self, id, secret, auth_cache):
        super().__init__(None)
        self.credentials = {
            "clientId": id,
            "clientSecret": secret,
        }
        self.auth_cache = auth_cache
    
    def setup(self):
        self.checkCredentials()
        return self

    def loadCache(self):
        return MockCache()

    def saveCache(self, _cache):
        pass
    
    def checkCredentials(self):
        if self.credentials['clientId'] == "" or self.credentials['clientSecret'] == "":
            self.enabled = False
            return

        try:
            cache_handler = CacheFileHandler(self.auth_cache)
            client_credentials_manager =SpotifyClientCredentials(
                client_id=self.credentials['clientId'],
                client_secret=self.credentials['clientSecret'],
                cache_handler=cache_handler
            )

            self.sp = spotipy.Spotify(client_credentials_manager=client_credentials_manager)
            self.sp.user_playlists('spotify')
            self.enabled = True
        except Exception:
            self.enabled = False 
