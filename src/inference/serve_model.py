import numpy as np
import socket
import sys
import tensorflow as tf
import os.path

from tensorflow.keras.preprocessing import image

IMG_HEIGHT = 350
IMG_WIDTH = 350


def start_server(model, port):
	""" Start a server on port listening for inference requests. """

	s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
	s.bind(('127.0.0.1', port))
	s.listen(1)
	conn, addr = s.accept()

	img_dir = os.path.dirname(os.path.abspath(__file__))

	while True:
		img_fname = conn.recv(1024)
		if not img_fname:
		    break
		img = image.load_img(os.path.join(img_dir, img_fname), target_size=(IMG_WIDTH, IMG_HEIGHT))
		img = image.img_to_array(img)
		img = np.expand_dims(img, axis=0)
		conn.sendall("{}".format(int(model.predict(img)[0][0])).encode('utf-8'))
	conn.close()


if __name__ == "__main__":
	if len(sys.argv) != 3:
		print("usage: python serve_model.py <saved-model-path> <port>")
	
	model = tf.keras.models.load_model(sys.argv[1])
	start_server(model, int(sys.argv[2]))
