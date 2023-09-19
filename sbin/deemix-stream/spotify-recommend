import spotipy
import click

SpotifyClientCredentials = spotipy.oauth2.SpotifyClientCredentials
CacheFileHandler = spotipy.cache_handler.CacheFileHandler

def checkCredentials(id, secret):
    cache_handler = CacheFileHandler(".auth-cache")
    
    client_credentials_manager = SpotifyClientCredentials(
        client_id=id,
        client_secret=secret,
        cache_handler=cache_handler
    )
    
    spotify = spotipy.Spotify(client_credentials_manager=client_credentials_manager)
    spotify.user_playlists('spotify')
    
    return spotify

@click.command()
@click.option('-s', '--spt-id', type=str, help='Path to the config folder')
@click.option('-ss', '--spt-secret', type=str, help='Path to the config folder')
@click.option('-sc', '--spt-cache', type=str, help='Path to the config folder')
@click.option('-l', '--limit', type=str, default="5", help='Path to the config folder')
@click.argument('isrcs', nargs=-1, required=True)
def main(spt_id, spt_secret, spt_cache, limit, isrcs):

    sp = checkCredentials(spt_id, spt_secret)
    tracks =  [ sp.search("isrc:"+x) for x in isrcs ]; 
    
    rec = sp.recommendations(seed_tracks=[
        x['tracks']['items'][0]["uri"] for x in tracks
    ], limit=limit)
    
    for x in rec["tracks"]:
        print(x["external_urls"]["spotify"])

if __name__ == '__main__':
    main(auto_envvar_prefix='DEEMIX')