str_try_cdrom_probe db '  Probe CD-ROM...',13,10,0
str_cdrom_ok_probe db '[OK] CD-ROM found',13,10,0
str_no_cdrom_probe db '[!!] No CD-ROM',13,10,0

; Shared log strings
str_log_nl          db 13,10,0
str_log_handoff     db '[  ] Handoff to boot image',13,10,0
str_cdrom_fail_stage db '[!!] CD-ROM boot stage failed',13,10,0
str_log_pvd_read    db '  Reading PVD...',13,10,0
str_err_pvd_sig     db '[!!] PVD signature mismatch',13,10,0
str_log_br_ok       db '[OK] Boot record found',13,10,0
str_log_cat_lba     db '  Catalog LBA=',0
str_log_cat_read    db '  Reading boot catalog...',13,10,0
str_err_cat_sig     db '[!!] Boot catalog signature mismatch',13,10,0
str_log_img_parse   db '  Parsing boot entry...',13,10,0
str_log_img_cnt     db '  Sector count=',0
str_log_img_lba     db '  Image LBA=',0
str_log_img_load    db '  Loading boot image...',13,10,0
