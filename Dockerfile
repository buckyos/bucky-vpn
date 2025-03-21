FROM nginx:latest

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY start.sh /start.sh
RUN chmod +x /start.sh
COPY vpn_web/build/web/ /usr/share/nginx/html
COPY target/x86_64-unknown-linux-musl/release/bucky-vpn-server /usr/share/bucky-vpn-server
RUN chmod +x /usr/share/bucky-vpn-server

EXPOSE 80

CMD ["/bin/sh", "/start.sh"]
