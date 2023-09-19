import spotipy
from spotipy.oauth2 import SpotifyOAuth

SpotifyClientCredentials = spotipy.oauth2.SpotifyClientCredentials

def checkCredentials(cachedir):
    cache_handler = CacheFileHandler(cachedir / ".auth-cache")
    
    client_credentials_manager = SpotifyClientCredentials(
        client_id=self.credentials['clientId'],
        client_secret=self.credentials['clientSecret'],
        cache_handler=cache_handler
    )
    
    spotify = spotipy.Spotify(client_credentials_manager=client_credentials_manager)
    spotify.user_playlists('spotify')
    
    return spotify


def getRecommendations(spotify, seeds={}, limit=10):
    recommendations = spotify.recommendations(limit=limit, **seeds)
    
    return recommendations