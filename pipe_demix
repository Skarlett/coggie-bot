#!/usr/bin/env python3
import click
import requests
import sys
from pathlib import Path

from deezer import Deezer
from deezer import TrackFormats
from deemix.types.Track import Track
from deemix import generateDownloadObject, parseLink
from deemix.settings import DEFAULTS as DEFAULT_SETTINGS, load as loadSettings
from deemix.utils import getBitrateNumberFromText, formatListener
import deemix.utils.localpaths as localpaths
from deemix.downloader import Downloader, getPreferredBitrate, extensions, formatsName, streamTrack, tagID3, tagFLAC
from deemix.itemgen import GenerationError
from deemix.itemgen import generateTrackItem
from deemix.errors import DownloadFailed, MD5NotFound, DownloadCanceled, PreferredBitrateNotFound, TrackNot360, AlbumDoesntExists, DownloadError, ErrorMessages
from deezer.errors import WrongLicense, WrongGeolocation

try:
    from deemix.plugins.spotify import Spotify
except ImportError:
    Spotify = None

dz = Deezer()
settings = DEFAULT_SETTINGS

def stream_stdout(downloader, extraData, track=None):
        returnData = {}
        trackAPI = extraData.get('trackAPI')
        albumAPI = extraData.get('albumAPI')
        playlistAPI = extraData.get('playlistAPI')
        trackAPI['size'] = downloader.downloadObject.size
        if downloader.downloadObject.isCanceled: raise DownloadCanceled
        if int(trackAPI['id']) == 0: raise DownloadFailed("notOnDeezer")

        itemData = {
            'id': trackAPI['id'],
            'title': trackAPI['title'],
            'artist': trackAPI['artist']['name']
        }

        # Create Track object
        if not track:
            downloader.log(itemData, "getTags")
            try:
                track = Track().parseData(
                    dz=downloader.dz,
                    track_id=trackAPI['id'],
                    trackAPI=trackAPI,
                    albumAPI=albumAPI,
                    playlistAPI=playlistAPI
                )
            except AlbumDoesntExists as e:
                raise DownloadError('albumDoesntExists') from e
            except MD5NotFound as e:
                raise DownloadError('notLoggedIn') from e
            downloader.log(itemData, "gotTags")

        itemData = {
            'id': track.id,
            'title': track.title,
            'artist': track.mainArtist.name
        }

        # Check if track not yet encoded
        if track.MD5 == '': raise DownloadFailed("notEncoded", track)

        # Choose the target bitrate
        downloader.log(itemData, "getBitrate")
        try:
            selectedFormat = getPreferredBitrate(
                downloader.dz,
                track,
                downloader.bitrate,
                downloader.settings['fallbackBitrate'], downloader.settings['feelingLucky'],
                downloader.downloadObject.uuid, downloader.listener
            )
        except WrongLicense as e:
            raise DownloadFailed("wrongLicense") from e
        except WrongGeolocation as e:
            raise DownloadFailed("wrongGeolocation", track) from e
        except PreferredBitrateNotFound as e:
            raise DownloadFailed("wrongBitrate", track) from e
        except TrackNot360 as e:
            raise DownloadFailed("no360RA") from e
        track.bitrate = selectedFormat
        track.album.bitrate = selectedFormat
        downloader.log(itemData, "gotBitrate")

        # Apply settings
        track.applySettings(downloader.settings)

        extension = extensions[track.bitrate]
        track.downloadURL = track.urls[formatsName[track.bitrate]]
        if not track.downloadURL: raise DownloadFailed('notAvailable', track)

        try:
            with open(sys.stdout.fileno(), 'wb') as stream:
                streamTrack(stream, track, downloadObject=downloader.downloadObject, listener=downloader.listener)
        except requests.exceptions.HTTPError as e:
            raise DownloadFailed('notAvailable', track) from e
        except OSError as e:
            if e.errno == errno.ENOSPC: raise DownloadFailed("noSpaceLeft") from e
            raise e

        downloader.log(itemData, "downloaded")
        downloader.downloadObject.completeTrackProgress(downloader.listener)

        if track.searched: returnData['searched'] = True

        downloader.downloadObject.downloaded += 1
        if downloader.listener: downloader.listener.send("updateQueue", {
            'uuid': downloader.downloadObject.uuid,
            'downloaded': True,
            'downloadPath': None,
            'extrasPath': str(downloader.downloadObject.extrasPath)
        })

@click.command()
@click.option('-b', '--bitrate', default=None, help='Overwrites the default bitrate selected')
@click.option('--arl', '-a', help='Deezer ARL Token')
@click.argument('url', nargs=-1, required=True)
def stream(url, arl, bitrate):
    assert arl, 'You must provide an ARL token'
    assert dz.login_via_arl(arl.strip()), 'Invalid ARL'
    if not bitrate: bitrate = settings.get("maxBitrate", TrackFormats.MP3_320)
    (link, link_type, link_id) = parseLink(url[0])
    obj = generateTrackItem(dz, link_id, bitrate)

    extraData = {
        'trackAPI': obj.single.get('trackAPI'),
        'albumAPI': obj.single.get('albumAPI')
    }

    downloader = Downloader(dz, obj, settings)
    stream_stdout(downloader, extraData)

if __name__ == '__main__':
    stream()
