use strict;
use warnings;
use ExtUtils::MakeMaker;

WriteMakefile(
    NAME         => 'Coggiebot::cleanupDL',
    AUTHOR       => q{Coggie <aaa>},
    VERSION_FROM => 'cleanup-downloads.pl',
    ABSTRACT     => 'Cleans files (which are not opened) for coggiebot',
    ( $ExtUtils::MakeMaker::VERSION >= 6.3002
        ? ( 'LICENSE' => 'perl' )
        : () ),
    PL_FILES  => {},
    PREREQ_PM => {
        'Set::Object'      => 1.40,
    }
);
