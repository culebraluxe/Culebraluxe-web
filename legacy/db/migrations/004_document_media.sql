alter table media
    drop constraint media_media_type_check;

alter table media
    add constraint media_media_type_check
    check (media_type in ('image', 'video', 'document'));

alter table property_media
    drop constraint property_media_role_check;

alter table property_media
    add constraint property_media_role_check
    check (role in ('hero', 'gallery', 'video', 'short', 'document'));
